import React, { useState, useEffect } from 'react';
import {
  View,
  Text,
  TouchableOpacity,
  StyleSheet,
  ScrollView,
  Platform,
  Alert,
  ActivityIndicator,
} from 'react-native';
import { useConfectOnboarding } from '../../../../packages/shared/src/hooks/useConfectOnboarding';
import { PermissionType } from '../../../../packages/shared/src/services/PermissionService';
import { DARK_THEME } from '../../constants/colors';

interface OnboardingScreenProps {
  onComplete: () => void;
}

export const OnboardingScreen: React.FC<OnboardingScreenProps> = ({ onComplete }) => {
  const [isLoading, setIsLoading] = useState(false);
  
  const onboarding = useConfectOnboarding({
    autoStartOnboarding: true,
    requiredPermissions: ['notifications', 'storage', 'network'],
  });

  const {
    onboardingState,
    isOnboardingComplete,
    updateOnboardingStep,
    completeOnboarding,
    checkPermissions,
    requestPermission,
    requestAllPermissions,
    getPermissionExplanation,
    canSkipStep,
    getNextStep,
  } = onboarding;

  // Auto-complete when onboarding is finished
  useEffect(() => {
    if (isOnboardingComplete) {
      onComplete();
    }
  }, [isOnboardingComplete, onComplete]);

  const handleContinue = async () => {
    setIsLoading(true);
    try {
      const nextStep = getNextStep(onboardingState.step);
      if (nextStep) {
        await updateOnboardingStep(nextStep, true);
      } else {
        await completeOnboarding();
      }
    } catch (error) {
      console.error('Failed to continue onboarding:', error);
      Alert.alert('Error', 'Failed to continue. Please try again.');
    } finally {
      setIsLoading(false);
    }
  };

  const handleSkip = async () => {
    if (!canSkipStep(onboardingState.step)) {
      return;
    }
    
    setIsLoading(true);
    try {
      const nextStep = getNextStep(onboardingState.step);
      if (nextStep) {
        await updateOnboardingStep(nextStep, false);
      } else {
        await completeOnboarding();
      }
    } catch (error) {
      console.error('Failed to skip onboarding step:', error);
      Alert.alert('Error', 'Failed to skip. Please try again.');
    } finally {
      setIsLoading(false);
    }
  };

  const handleRequestPermissions = async () => {
    setIsLoading(true);
    try {
      const results = await requestAllPermissions();
      const allGranted = results.every(result => result.status === 'granted');
      
      if (allGranted) {
        await handleContinue();
      } else {
        // Show permission results
        const deniedPermissions = results.filter(r => r.status === 'denied');
        if (deniedPermissions.length > 0) {
          Alert.alert(
            'Permissions',
            `Some permissions were denied. The app will use fallback functionality for: ${deniedPermissions.map(p => p.type).join(', ')}`,
            [{ text: 'Continue', onPress: handleContinue }]
          );
        } else {
          await handleContinue();
        }
      }
    } catch (error) {
      console.error('Failed to request permissions:', error);
      Alert.alert('Error', 'Failed to request permissions. Please try again.');
    } finally {
      setIsLoading(false);
    }
  };

  const renderWelcomeStep = () => (
    <View style={styles.stepContainer}>
      <Text style={styles.stepTitle}>Welcome to Claude Code</Text>
      <Text style={styles.stepDescription}>
        Claude Code brings AI-powered development assistance to your mobile device. 
        Let's get you set up in just a few steps.
      </Text>
      <View style={styles.featureList}>
        <Text style={styles.featureItem}>• Seamless sync with desktop sessions</Text>
        <Text style={styles.featureItem}>• Real-time collaboration with Claude</Text>
        <Text style={styles.featureItem}>• Cross-platform code analysis</Text>
        <Text style={styles.featureItem}>• Smart project management</Text>
      </View>
    </View>
  );

  const renderPermissionsStep = () => (
    <View style={styles.stepContainer}>
      <Text style={styles.stepTitle}>App Permissions</Text>
      <Text style={styles.stepDescription}>
        Claude Code needs a few permissions to provide the best experience:
      </Text>
      <View style={styles.permissionsList}>
        {(['notifications', 'storage', 'network'] as PermissionType[]).map((permission) => (
          <View key={permission} style={styles.permissionItem}>
            <Text style={styles.permissionName}>
              {permission.charAt(0).toUpperCase() + permission.slice(1)}
            </Text>
            <Text style={styles.permissionDescription}>
              {getPermissionExplanation(permission)}
            </Text>
          </View>
        ))}
      </View>
      <TouchableOpacity
        style={styles.primaryButton}
        onPress={handleRequestPermissions}
        disabled={isLoading}
      >
        {isLoading ? (
          <ActivityIndicator color="#ffffff" />
        ) : (
          <Text style={styles.primaryButtonText}>Grant Permissions</Text>
        )}
      </TouchableOpacity>
    </View>
  );

  const renderGitHubStep = () => (
    <View style={styles.stepContainer}>
      <Text style={styles.stepTitle}>GitHub Connected</Text>
      <Text style={styles.stepDescription}>
        Your GitHub account is successfully connected! This enables:
      </Text>
      <View style={styles.featureList}>
        <Text style={styles.featureItem}>• Access to your repositories</Text>
        <Text style={styles.featureItem}>• Secure authentication</Text>
        <Text style={styles.featureItem}>• Synchronized project settings</Text>
      </View>
      <Text style={styles.connectedUser}>
        Connected as: {onboarding.user?.githubUsername || 'Unknown'}
      </Text>
    </View>
  );

  const renderRepositoryStep = () => (
    <View style={styles.stepContainer}>
      <Text style={styles.stepTitle}>Select Repository</Text>
      <Text style={styles.stepDescription}>
        Choose a repository to work with, or skip this step to set it up later.
      </Text>
      <View style={styles.repositoryOptions}>
        <TouchableOpacity style={styles.repositoryOption}>
          <Text style={styles.repositoryName}>Browse Repositories</Text>
          <Text style={styles.repositoryDescription}>
            Select from your GitHub repositories
          </Text>
        </TouchableOpacity>
      </View>
      <Text style={styles.skipText}>
        You can set up repository access later in settings.
      </Text>
    </View>
  );

  const renderPreferencesStep = () => (
    <View style={styles.stepContainer}>
      <Text style={styles.stepTitle}>Preferences</Text>
      <Text style={styles.stepDescription}>
        Customize your Claude Code experience:
      </Text>
      <View style={styles.preferencesList}>
        <View style={styles.preferenceItem}>
          <Text style={styles.preferenceName}>Theme</Text>
          <Text style={styles.preferenceValue}>Dark (System)</Text>
        </View>
        <View style={styles.preferenceItem}>
          <Text style={styles.preferenceName}>Notifications</Text>
          <Text style={styles.preferenceValue}>Enabled</Text>
        </View>
        <View style={styles.preferenceItem}>
          <Text style={styles.preferenceName}>Auto Sync</Text>
          <Text style={styles.preferenceValue}>Enabled</Text>
        </View>
      </View>
      <Text style={styles.skipText}>
        You can change these settings anytime in the app.
      </Text>
    </View>
  );

  const renderStepContent = () => {
    switch (onboardingState.step) {
      case 'welcome':
        return renderWelcomeStep();
      case 'permissions_explained':
        return renderPermissionsStep();
      case 'github_connected':
        return renderGitHubStep();
      case 'repository_selected':
        return renderRepositoryStep();
      case 'preferences_set':
        return renderPreferencesStep();
      default:
        return renderWelcomeStep();
    }
  };

  const getCurrentStepNumber = () => {
    const steps = ['welcome', 'permissions_explained', 'github_connected', 'repository_selected', 'preferences_set'];
    return steps.indexOf(onboardingState.step) + 1;
  };

  const getTotalSteps = () => 5;

  if (onboardingState.isLoading) {
    return (
      <View style={styles.loadingContainer}>
        <ActivityIndicator size="large" color="#60a5fa" />
        <Text style={styles.loadingText}>Setting up your experience...</Text>
      </View>
    );
  }

  return (
    <View style={styles.container}>
      <ScrollView style={styles.scrollContainer} showsVerticalScrollIndicator={false}>
        <View style={styles.header}>
          <Text style={styles.stepCounter}>
            Step {getCurrentStepNumber()} of {getTotalSteps()}
          </Text>
          <View style={styles.progressBar}>
            <View 
              style={[
                styles.progressFill, 
                { width: `${(getCurrentStepNumber() / getTotalSteps()) * 100}%` }
              ]} 
            />
          </View>
        </View>

        {renderStepContent()}

        {onboardingState.error && (
          <View style={styles.errorContainer}>
            <Text style={styles.errorText}>{onboardingState.error}</Text>
          </View>
        )}
      </ScrollView>

      <View style={styles.buttonContainer}>
        {canSkipStep(onboardingState.step) && (
          <TouchableOpacity
            style={styles.skipButton}
            onPress={handleSkip}
            disabled={isLoading}
          >
            <Text style={styles.skipButtonText}>Skip</Text>
          </TouchableOpacity>
        )}
        
        {onboardingState.step !== 'permissions_explained' && (
          <TouchableOpacity
            style={[styles.continueButton, isLoading && styles.disabledButton]}
            onPress={handleContinue}
            disabled={isLoading}
          >
            {isLoading ? (
              <ActivityIndicator color="#ffffff" />
            ) : (
              <Text style={styles.continueButtonText}>Continue</Text>
            )}
          </TouchableOpacity>
        )}
      </View>
    </View>
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
    backgroundColor: DARK_THEME.background,
  },
  loadingText: {
    color: DARK_THEME.text,
    fontSize: 16,
    marginTop: 16,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  scrollContainer: {
    flex: 1,
  },
  header: {
    padding: 24,
    paddingBottom: 16,
  },
  stepCounter: {
    color: '#a1a1aa',
    fontSize: 14,
    marginBottom: 12,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  progressBar: {
    height: 4,
    backgroundColor: '#27272a',
    borderRadius: 2,
  },
  progressFill: {
    height: '100%',
    backgroundColor: '#60a5fa',
    borderRadius: 2,
  },
  stepContainer: {
    padding: 24,
    paddingTop: 8,
  },
  stepTitle: {
    color: DARK_THEME.text,
    fontSize: 28,
    fontWeight: 'bold',
    marginBottom: 16,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  stepDescription: {
    color: '#a1a1aa',
    fontSize: 16,
    lineHeight: 24,
    marginBottom: 24,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  featureList: {
    marginBottom: 24,
  },
  featureItem: {
    color: '#d4d4d8',
    fontSize: 16,
    lineHeight: 24,
    marginBottom: 8,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  permissionsList: {
    marginBottom: 24,
  },
  permissionItem: {
    backgroundColor: '#18181b',
    borderRadius: 8,
    padding: 16,
    marginBottom: 12,
    borderLeftWidth: 4,
    borderLeftColor: '#60a5fa',
  },
  permissionName: {
    color: DARK_THEME.text,
    fontSize: 16,
    fontWeight: 'bold',
    marginBottom: 4,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  permissionDescription: {
    color: '#a1a1aa',
    fontSize: 14,
    lineHeight: 20,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  connectedUser: {
    color: '#22c55e',
    fontSize: 16,
    fontWeight: 'bold',
    textAlign: 'center',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  repositoryOptions: {
    marginBottom: 24,
  },
  repositoryOption: {
    backgroundColor: '#18181b',
    borderRadius: 8,
    padding: 16,
    borderWidth: 1,
    borderColor: '#27272a',
  },
  repositoryName: {
    color: DARK_THEME.text,
    fontSize: 16,
    fontWeight: 'bold',
    marginBottom: 4,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  repositoryDescription: {
    color: '#a1a1aa',
    fontSize: 14,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  preferencesList: {
    marginBottom: 24,
  },
  preferenceItem: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    backgroundColor: '#18181b',
    borderRadius: 8,
    padding: 16,
    marginBottom: 8,
  },
  preferenceName: {
    color: DARK_THEME.text,
    fontSize: 16,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  preferenceValue: {
    color: '#60a5fa',
    fontSize: 16,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  skipText: {
    color: '#71717a',
    fontSize: 14,
    textAlign: 'center',
    fontStyle: 'italic',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  errorContainer: {
    margin: 24,
    padding: 16,
    backgroundColor: '#7f1d1d',
    borderRadius: 8,
    borderLeftWidth: 4,
    borderLeftColor: '#dc2626',
  },
  errorText: {
    color: '#fecaca',
    fontSize: 14,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  buttonContainer: {
    flexDirection: 'row',
    padding: 24,
    paddingTop: 16,
    gap: 12,
  },
  skipButton: {
    flex: 1,
    paddingVertical: 16,
    paddingHorizontal: 24,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: '#27272a',
    alignItems: 'center',
  },
  skipButtonText: {
    color: '#a1a1aa',
    fontSize: 16,
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  continueButton: {
    flex: 2,
    backgroundColor: '#60a5fa',
    paddingVertical: 16,
    paddingHorizontal: 24,
    borderRadius: 8,
    alignItems: 'center',
  },
  continueButtonText: {
    color: '#ffffff',
    fontSize: 16,
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  primaryButton: {
    backgroundColor: '#22c55e',
    paddingVertical: 16,
    paddingHorizontal: 24,
    borderRadius: 8,
    alignItems: 'center',
  },
  primaryButtonText: {
    color: '#ffffff',
    fontSize: 16,
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  disabledButton: {
    backgroundColor: '#374151',
  },
});