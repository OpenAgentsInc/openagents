import React, { useState, useEffect } from 'react';
import {
  View,
  Text,
  TouchableOpacity,
  StyleSheet,
  Platform,
  ActivityIndicator,
  Animated,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { DARK_THEME } from '../../constants/colors';
import { useConfectAuth } from '../../contexts/SimpleConfectAuthContext';

// Define device sync types
interface DeviceConnection {
  deviceId: string;
  userId: string;
  deviceInfo: {
    deviceType: 'desktop' | 'mobile' | 'web';
    platform: string;
    appVersion: string;
    userAgent?: string;
    lastSeen: number;
    capabilities: string[];
  };
  status: 'online' | 'offline' | 'idle';
  sessionToken: string;
  roomToken: string;
  connectedAt: number;
  lastHeartbeat: number;
}

interface DeviceSyncOnboardingScreenProps {
  onContinue: () => void;
  onSkip: () => void;
  canSkip?: boolean;
}

// Device avatar component for the facepile
const DeviceAvatar: React.FC<{
  device: DeviceConnection;
  index: number;
  total: number;
}> = ({ device, index, total }) => {
  const fadeAnim = new Animated.Value(0);

  useEffect(() => {
    // Stagger the animation for each device
    Animated.timing(fadeAnim, {
      toValue: 1,
      duration: 300,
      delay: index * 150,
      useNativeDriver: true,
    }).start();
  }, [fadeAnim, index]);

  const getDeviceIcon = (deviceType: string, platform: string) => {
    if (deviceType === "desktop") {
      return platform.toLowerCase().includes("mac") ? "💻" : "🖥️";
    }
    if (deviceType === "mobile") {
      return platform.toLowerCase().includes("ios") ? "📱" : "📱";
    }
    return "🌐"; // web
  };

  const getDeviceName = (deviceType: string, platform: string) => {
    if (deviceType === "desktop") {
      if (platform.toLowerCase().includes("mac")) return "Mac";
      if (platform.toLowerCase().includes("windows")) return "Windows PC";
      if (platform.toLowerCase().includes("linux")) return "Linux";
      return "Desktop";
    }
    if (deviceType === "mobile") {
      if (platform.toLowerCase().includes("ios")) return "iPhone";
      if (platform.toLowerCase().includes("android")) return "Android";
      return "Mobile";
    }
    return "Web Browser";
  };

  return (
    <Animated.View 
      style={[
        styles.deviceAvatar,
        {
          opacity: fadeAnim,
          zIndex: total - index,
          marginLeft: index > 0 ? -12 : 0,
        }
      ]}
    >
      <View style={[
        styles.avatarCircle,
        device.status === "online" ? styles.onlineIndicator : styles.offlineIndicator
      ]}>
        <Text style={styles.deviceIcon}>
          {getDeviceIcon(device.deviceInfo.deviceType, device.deviceInfo.platform)}
        </Text>
      </View>
      <View style={styles.deviceTooltip}>
        <Text style={styles.deviceName}>
          {getDeviceName(device.deviceInfo.deviceType, device.deviceInfo.platform)}
        </Text>
        <Text style={styles.deviceStatus}>
          {device.status === "online" ? "Connected" : "Offline"}
        </Text>
      </View>
    </Animated.View>
  );
};

// Device connection facepile component
const DeviceConnectionFacePile: React.FC<{
  devices: DeviceConnection[];
  maxVisible?: number;
}> = ({ devices, maxVisible = 5 }) => {
  const visible = devices.slice(0, maxVisible);
  const hidden = devices.slice(maxVisible);

  if (devices.length === 0) {
    return (
      <View style={styles.emptyDeviceContainer}>
        <Text style={styles.emptyDeviceIcon}>🔍</Text>
        <Text style={styles.emptyDeviceText}>
          No desktop devices detected
        </Text>
      </View>
    );
  }

  return (
    <View style={styles.facepileContainer}>
      <View style={styles.avatarsContainer}>
        {visible.map((device, idx) => (
          <DeviceAvatar 
            key={device.deviceId} 
            device={device} 
            index={idx}
            total={visible.length}
          />
        ))}
        {hidden.length > 0 && (
          <View style={styles.moreDevicesIndicator}>
            <Text style={styles.moreDevicesText}>+{hidden.length}</Text>
          </View>
        )}
      </View>
      <Text style={styles.deviceCountText}>
        {devices.length} device{devices.length !== 1 ? 's' : ''} connected
      </Text>
    </View>
  );
};

// Main component
export const DeviceSyncOnboardingScreen: React.FC<DeviceSyncOnboardingScreenProps> = ({
  onContinue,
  onSkip,
  canSkip = true,
}) => {
  const { user } = useConfectAuth();
  const [connectedDevices, setConnectedDevices] = useState<DeviceConnection[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isWaiting, setIsWaiting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Pulse animation for waiting state
  const pulseAnim = new Animated.Value(1);

  useEffect(() => {
    if (isWaiting) {
      const pulse = () => {
        Animated.sequence([
          Animated.timing(pulseAnim, {
            toValue: 0.7,
            duration: 1000,
            useNativeDriver: true,
          }),
          Animated.timing(pulseAnim, {
            toValue: 1,
            duration: 1000,
            useNativeDriver: true,
          }),
        ]).start(() => {
          if (isWaiting) pulse();
        });
      };
      pulse();
    }
  }, [isWaiting, pulseAnim]);

  // Simulate device detection (in real implementation, this would use useDevicePresence hook)
  useEffect(() => {
    const detectDevices = async () => {
      setIsLoading(true);
      try {
        // Simulate loading time
        await new Promise(resolve => setTimeout(resolve, 1500));
        
        // Mock connected devices (in real implementation, this would come from Convex)
        const mockDevices: DeviceConnection[] = [
          {
            deviceId: 'desktop_macos_001',
            userId: user?.id || 'mock_user',
            deviceInfo: {
              deviceType: 'desktop',
              platform: 'macos',
              appVersion: '1.0.0',
              lastSeen: Date.now(),
              capabilities: ['claude-code', 'file-sync'],
            },
            status: 'online',
            sessionToken: 'mock_session_token',
            roomToken: `room_${user?.id}`,
            connectedAt: Date.now() - 60000,
            lastHeartbeat: Date.now(),
          },
        ];

        setConnectedDevices(mockDevices);
        setIsWaiting(mockDevices.length === 0);
      } catch (err) {
        setError('Failed to detect devices. Please try again.');
        console.error('Device detection error:', err);
      } finally {
        setIsLoading(false);
      }
    };

    detectDevices();
  }, [user?.id]);

  const hasDesktopConnected = connectedDevices.some(
    device => device.deviceInfo.deviceType === 'desktop' && device.status === 'online'
  );

  const handleContinue = () => {
    console.log('📱 [DEVICE-SYNC] Continuing with connected devices:', connectedDevices.length);
    onContinue();
  };

  const handleSkip = () => {
    console.log('📱 [DEVICE-SYNC] Skipping device sync');
    onSkip();
  };

  const handleRetryDetection = () => {
    setError(null);
    setIsLoading(true);
    // Trigger device detection again
    setTimeout(() => {
      setIsLoading(false);
      setIsWaiting(true);
    }, 1000);
  };

  if (isLoading) {
    return (
      <SafeAreaView style={styles.container}>
        <View style={styles.loadingContainer}>
          <ActivityIndicator size="large" color={DARK_THEME.primary} />
          <Text style={styles.loadingText}>
            Detecting connected devices...
          </Text>
        </View>
      </SafeAreaView>
    );
  }

  return (
    <SafeAreaView style={styles.container}>
      <View style={styles.header}>
        <Text style={styles.stepTitle}>Connect Your Desktop</Text>
        <Text style={styles.stepDescription}>
          This app works best when connected to Claude Code running on your desktop computer.
        </Text>
      </View>

      <View style={styles.content}>
        {error ? (
          <View style={styles.errorContainer}>
            <Text style={styles.errorIcon}>⚠️</Text>
            <Text style={styles.errorText}>{error}</Text>
            <TouchableOpacity 
              style={styles.retryButton}
              onPress={handleRetryDetection}
            >
              <Text style={styles.retryButtonText}>Try Again</Text>
            </TouchableOpacity>
          </View>
        ) : (
          <>
            <DeviceConnectionFacePile devices={connectedDevices} />

            {isWaiting ? (
              <Animated.View 
                style={[
                  styles.waitingContainer,
                  { opacity: pulseAnim }
                ]}
              >
                <ActivityIndicator size="small" color={DARK_THEME.primary} />
                <Text style={styles.waitingText}>
                  Waiting for desktop connection...
                </Text>
                <Text style={styles.instructionText}>
                  Open Claude Code on your desktop to continue
                </Text>
              </Animated.View>
            ) : hasDesktopConnected ? (
              <View style={styles.successContainer}>
                <Text style={styles.successIcon}>✅</Text>
                <Text style={styles.successText}>
                  Desktop connected successfully!
                </Text>
                <Text style={styles.successSubtext}>
                  You can now sync sessions between your devices.
                </Text>
              </View>
            ) : (
              <View style={styles.infoContainer}>
                <Text style={styles.infoText}>
                  Connect your desktop for the best experience:
                </Text>
                <View style={styles.featureList}>
                  <Text style={styles.featureItem}>• Real-time session sync</Text>
                  <Text style={styles.featureItem}>• Cross-device continuity</Text>
                  <Text style={styles.featureItem}>• Shared project context</Text>
                </View>
              </View>
            )}
          </>
        )}
      </View>

      <View style={styles.buttonContainer}>
        {hasDesktopConnected ? (
          <TouchableOpacity
            style={styles.primaryButton}
            onPress={handleContinue}
          >
            <Text style={styles.primaryButtonText}>
              Continue with Connected Desktop
            </Text>
          </TouchableOpacity>
        ) : (
          <TouchableOpacity
            style={styles.waitButton}
            onPress={handleRetryDetection}
          >
            <Text style={styles.waitButtonText}>
              🔄 Check Again
            </Text>
          </TouchableOpacity>
        )}

        {canSkip && (
          <TouchableOpacity
            style={styles.skipButton}
            onPress={handleSkip}
          >
            <Text style={styles.skipButtonText}>
              Skip for now
            </Text>
          </TouchableOpacity>
        )}
      </View>
    </SafeAreaView>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: DARK_THEME.background,
  },
  loadingContainer: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: 24,
  },
  loadingText: {
    color: DARK_THEME.textSecondary,
    fontSize: 16,
    marginTop: 16,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    }),
  },
  header: {
    padding: 24,
    paddingBottom: 16,
  },
  stepTitle: {
    color: DARK_THEME.text,
    fontSize: 28,
    fontWeight: 'bold',
    marginBottom: 16,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    }),
  },
  stepDescription: {
    color: DARK_THEME.textSecondary,
    fontSize: 16,
    lineHeight: 24,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    }),
  },
  content: {
    flex: 1,
    padding: 24,
    paddingTop: 8,
  },
  // Device facepile styles
  facepileContainer: {
    alignItems: 'center',
    marginBottom: 32,
  },
  avatarsContainer: {
    flexDirection: 'row',
    alignItems: 'center',
    marginBottom: 16,
  },
  deviceAvatar: {
    alignItems: 'center',
  },
  avatarCircle: {
    width: 64,
    height: 64,
    borderRadius: 32,
    justifyContent: 'center',
    alignItems: 'center',
    borderWidth: 3,
    backgroundColor: DARK_THEME.backgroundSecondary,
  },
  onlineIndicator: {
    borderColor: '#10b981', // green-500
  },
  offlineIndicator: {
    borderColor: DARK_THEME.border,
  },
  deviceIcon: {
    fontSize: 24,
  },
  deviceTooltip: {
    marginTop: 8,
    alignItems: 'center',
  },
  deviceName: {
    color: DARK_THEME.text,
    fontSize: 12,
    fontWeight: '600',
    marginBottom: 2,
  },
  deviceStatus: {
    color: DARK_THEME.textTertiary,
    fontSize: 10,
  },
  moreDevicesIndicator: {
    width: 48,
    height: 48,
    borderRadius: 24,
    backgroundColor: DARK_THEME.border,
    justifyContent: 'center',
    alignItems: 'center',
    marginLeft: -12,
  },
  moreDevicesText: {
    color: DARK_THEME.text,
    fontSize: 12,
    fontWeight: '600',
  },
  deviceCountText: {
    color: DARK_THEME.textSecondary,
    fontSize: 14,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    }),
  },
  // Empty state styles
  emptyDeviceContainer: {
    alignItems: 'center',
    marginBottom: 32,
  },
  emptyDeviceIcon: {
    fontSize: 48,
    marginBottom: 16,
  },
  emptyDeviceText: {
    color: DARK_THEME.textSecondary,
    fontSize: 16,
    textAlign: 'center',
  },
  // Status container styles
  waitingContainer: {
    alignItems: 'center',
    marginBottom: 32,
  },
  waitingText: {
    color: DARK_THEME.text,
    fontSize: 18,
    fontWeight: '600',
    marginTop: 16,
    textAlign: 'center',
  },
  instructionText: {
    color: DARK_THEME.textSecondary,
    fontSize: 14,
    marginTop: 8,
    textAlign: 'center',
    lineHeight: 20,
  },
  successContainer: {
    alignItems: 'center',
    marginBottom: 32,
  },
  successIcon: {
    fontSize: 48,
    marginBottom: 16,
  },
  successText: {
    color: '#10b981', // green-500
    fontSize: 18,
    fontWeight: '600',
    marginBottom: 8,
    textAlign: 'center',
  },
  successSubtext: {
    color: DARK_THEME.textSecondary,
    fontSize: 14,
    textAlign: 'center',
    lineHeight: 20,
  },
  infoContainer: {
    marginBottom: 32,
  },
  infoText: {
    color: DARK_THEME.text,
    fontSize: 16,
    marginBottom: 16,
    textAlign: 'center',
  },
  featureList: {
    alignItems: 'flex-start',
  },
  featureItem: {
    color: DARK_THEME.textSecondary,
    fontSize: 14,
    lineHeight: 20,
    marginBottom: 4,
  },
  // Error styles
  errorContainer: {
    alignItems: 'center',
    marginBottom: 32,
  },
  errorIcon: {
    fontSize: 48,
    marginBottom: 16,
  },
  errorText: {
    color: '#ef4444', // red-500
    fontSize: 16,
    textAlign: 'center',
    marginBottom: 16,
  },
  retryButton: {
    backgroundColor: DARK_THEME.backgroundSecondary,
    paddingHorizontal: 16,
    paddingVertical: 8,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: DARK_THEME.border,
  },
  retryButtonText: {
    color: DARK_THEME.text,
    fontSize: 14,
    fontWeight: '600',
  },
  // Button styles
  buttonContainer: {
    padding: 24,
    paddingTop: 16,
  },
  primaryButton: {
    backgroundColor: DARK_THEME.primary,
    paddingVertical: 16,
    borderRadius: 12,
    alignItems: 'center',
    marginBottom: 12,
  },
  primaryButtonText: {
    color: '#ffffff',
    fontSize: 16,
    fontWeight: '600',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    }),
  },
  waitButton: {
    backgroundColor: DARK_THEME.backgroundSecondary,
    paddingVertical: 16,
    borderRadius: 12,
    alignItems: 'center',
    marginBottom: 12,
    borderWidth: 1,
    borderColor: DARK_THEME.border,
  },
  waitButtonText: {
    color: DARK_THEME.text,
    fontSize: 16,
    fontWeight: '600',
  },
  skipButton: {
    paddingVertical: 16,
    alignItems: 'center',
  },
  skipButtonText: {
    color: DARK_THEME.textSecondary,
    fontSize: 16,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    }),
  },
});