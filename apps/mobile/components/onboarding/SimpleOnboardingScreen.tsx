import React, { useState } from 'react';
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
import { useConfectAuth } from '../../contexts/SimpleConfectAuthContext';
import { DARK_THEME } from '../../constants/colors';

interface SimpleOnboardingScreenProps {
  onComplete: () => void;
}

export const SimpleOnboardingScreen: React.FC<SimpleOnboardingScreenProps> = ({ onComplete }) => {
  const [currentStep, setCurrentStep] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const { user, markOnboardingComplete } = useConfectAuth();

  const steps = [
    {
      title: 'Welcome to Claude Code',
      description: 'Claude Code brings AI-powered development assistance to your mobile device.',
      content: (
        <View style={styles.featureList}>
          <Text style={styles.featureItem}>• Seamless sync with desktop sessions</Text>
          <Text style={styles.featureItem}>• Real-time collaboration with Claude</Text>
          <Text style={styles.featureItem}>• Cross-platform code analysis</Text>
          <Text style={styles.featureItem}>• Smart project management</Text>
        </View>
      ),
    },
    {
      title: 'Permissions Setup',
      description: 'Grant permissions for the best experience.',
      content: (
        <View style={styles.permissionsList}>
          <View style={styles.permissionItem}>
            <Text style={styles.permissionName}>Notifications</Text>
            <Text style={styles.permissionDescription}>
              Get notified about important updates and messages
            </Text>
          </View>
          <View style={styles.permissionItem}>
            <Text style={styles.permissionName}>Storage</Text>
            <Text style={styles.permissionDescription}>
              Save your preferences and session data
            </Text>
          </View>
          <View style={styles.permissionItem}>
            <Text style={styles.permissionName}>Network</Text>
            <Text style={styles.permissionDescription}>
              Sync with cloud and communicate with Claude
            </Text>
          </View>
        </View>
      ),
    },
    {
      title: 'GitHub Connected',
      description: 'Your GitHub account is successfully connected!',
      content: (
        <View style={styles.connectionInfo}>
          <Text style={styles.connectedUser}>
            Connected as: {user?.githubUsername || 'Loading...'}
          </Text>
          <View style={styles.featureList}>
            <Text style={styles.featureItem}>• Access to your repositories</Text>
            <Text style={styles.featureItem}>• Secure authentication</Text>
            <Text style={styles.featureItem}>• Synchronized project settings</Text>
          </View>
        </View>
      ),
    },
    {
      title: 'You\'re All Set!',
      description: 'Welcome to Claude Code. You can now start creating sessions and collaborating with Claude.',
      content: (
        <View style={styles.completionContent}>
          <Text style={styles.completionText}>
            🎉 Onboarding complete! You can now:
          </Text>
          <View style={styles.featureList}>
            <Text style={styles.featureItem}>• Create new Claude Code sessions</Text>
            <Text style={styles.featureItem}>• Sync with desktop app</Text>
            <Text style={styles.featureItem}>• Manage your projects</Text>
            <Text style={styles.featureItem}>• Configure preferences</Text>
          </View>
        </View>
      ),
    },
  ];

  const handleContinue = async () => {
    if (currentStep < steps.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      setIsLoading(true);
      // Mark onboarding as complete
      await markOnboardingComplete();
      setTimeout(() => {
        setIsLoading(false);
        onComplete();
      }, 1000);
    }
  };

  const handleSkip = () => {
    if (currentStep < steps.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      onComplete();
    }
  };

  const currentStepData = steps[currentStep];
  const isLastStep = currentStep === steps.length - 1;

  return (
    <View style={styles.container}>
      <ScrollView style={styles.scrollContainer} showsVerticalScrollIndicator={false}>
        <View style={styles.header}>
          <Text style={styles.stepCounter}>
            Step {currentStep + 1} of {steps.length}
          </Text>
          <View style={styles.progressBar}>
            <View 
              style={[
                styles.progressFill, 
                { width: `${((currentStep + 1) / steps.length) * 100}%` }
              ]} 
            />
          </View>
        </View>

        <View style={styles.stepContainer}>
          <Text style={styles.stepTitle}>{currentStepData.title}</Text>
          <Text style={styles.stepDescription}>{currentStepData.description}</Text>
          {currentStepData.content}
        </View>
      </ScrollView>

      <View style={styles.buttonContainer}>
        {!isLastStep && (
          <TouchableOpacity
            style={styles.skipButton}
            onPress={handleSkip}
            disabled={isLoading}
          >
            <Text style={styles.skipButtonText}>Skip</Text>
          </TouchableOpacity>
        )}
        
        <TouchableOpacity
          style={[styles.continueButton, isLoading && styles.disabledButton]}
          onPress={handleContinue}
          disabled={isLoading}
        >
          {isLoading ? (
            <ActivityIndicator color="#ffffff" />
          ) : (
            <Text style={styles.continueButtonText}>
              {isLastStep ? 'Get Started' : 'Continue'}
            </Text>
          )}
        </TouchableOpacity>
      </View>
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: DARK_THEME.background,
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
  connectionInfo: {
    alignItems: 'center',
  },
  connectedUser: {
    color: '#22c55e',
    fontSize: 18,
    fontWeight: 'bold',
    marginBottom: 24,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  completionContent: {
    alignItems: 'center',
  },
  completionText: {
    color: '#22c55e',
    fontSize: 18,
    fontWeight: 'bold',
    marginBottom: 24,
    textAlign: 'center',
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
  disabledButton: {
    backgroundColor: '#374151',
  },
});