import React from 'react';
import {
  View,
  TouchableOpacity,
  StyleSheet,
  Platform,
  Alert,
} from 'react-native';
import { useQuery } from 'convex/react';
import { api } from '../../convex/_generated/api';
import { Text, ErrorBoundary } from '../index';
import { useConfectAuth } from '../../contexts/SimpleConfectAuthContext';
import { useRepositoryState } from '../../hooks/useRepositoryState';

interface Repository {
  url: string;
  name: string;
  owner: string;
  isPrivate: boolean;
  defaultBranch?: string;
}

interface NewSessionScreenProps {
  onCreateSession: () => void;
  onChangeRepository?: () => void;
}

/**
 * New Session Screen shown after repository selection.
 * Displays a centered "New Session" button with active repository info.
 */
export function NewSessionScreen({ 
  onCreateSession, 
  onChangeRepository 
}: NewSessionScreenProps) {
  const { isAuthenticated, user } = useConfectAuth();
  
  // Repository state management
  const { activeRepository } = useRepositoryState();

  // Query onboarding progress for additional data if needed
  const onboardingProgress = useQuery(
    api.confect.onboarding.getOnboardingProgress,
    isAuthenticated ? {} : "skip"
  );

  // Helper function to extract data from Effect Option type
  const extractOnboardingData = (optionData: any) => {
    if (!optionData || typeof optionData !== 'object') return null;
    if (optionData._tag === 'Some' && optionData.value) return optionData.value;
    if (optionData._tag === 'None') return null;
    // If it's already unwrapped data (fallback)
    if (optionData.step) return optionData;
    return null;
  };

  const actualOnboardingData = extractOnboardingData(onboardingProgress);

  // Debug logging
  React.useEffect(() => {
    const timestamp = new Date().toISOString();
    console.log(`🔍 [NEW_SESSION] ${timestamp} Component state:`, {
      isAuthenticated,
      user: user?.githubUsername,
      activeRepository: activeRepository ? `${activeRepository.owner}/${activeRepository.name}` : 'null/undefined',
      hasActiveRepository: !!activeRepository,
      activeRepositoryRaw: activeRepository,
      onboardingProgressStep: actualOnboardingData?.step,
      onboardingProgressActiveRepo: actualOnboardingData?.activeRepository,
      buttonDisabled: !activeRepository,
      onboardingRaw: onboardingProgress,
    });
  }, [isAuthenticated, user, activeRepository, onboardingProgress, actualOnboardingData]);

  // Handle create session
  const handleCreateSession = () => {
    const timestamp = new Date().toISOString();
    console.log(`🚀 [NEW_SESSION] ${timestamp} User initiated new session creation`, {
      activeRepository: activeRepository ? `${activeRepository.owner}/${activeRepository.name}` : 'none',
      user: user?.githubUsername,
    });

    if (!activeRepository) {
      Alert.alert(
        'No Repository Selected',
        'Please select a repository before creating a session.',
        [
          { text: 'Select Repository', onPress: onChangeRepository },
          { text: 'Cancel', style: 'cancel' },
        ]
      );
      return;
    }

    onCreateSession();
  };

  // Handle repository change
  const handleChangeRepository = () => {
    const timestamp = new Date().toISOString();
    console.log(`🔄 [NEW_SESSION] ${timestamp} User requested to change repository`);
    onChangeRepository?.();
  };

  // Loading state
  if (!isAuthenticated) {
    return (
      <View style={styles.container}>
        <View style={styles.centerContainer}>
          <Text style={styles.errorText}>Please log in to continue</Text>
        </View>
      </View>
    );
  }

  // Loading onboarding data
  if (!onboardingProgress) {
    return (
      <View style={styles.container}>
        <View style={styles.centerContainer}>
          <Text style={styles.loadingText}>Loading...</Text>
        </View>
      </View>
    );
  }

  return (
    <ErrorBoundary
      onError={(error, errorInfo) => {
        const timestamp = new Date().toISOString();
        console.error(`❌ [NEW_SESSION] ${timestamp} Component error:`, {
          error: error.message,
          stack: error.stack,
          componentStack: errorInfo.componentStack,
          user: user?.githubUsername,
          activeRepository: activeRepository ? `${activeRepository.owner}/${activeRepository.name}` : null,
        });
      }}
    >
      <View style={styles.container}>
        <View style={styles.centerContainer}>
          <View style={styles.headerContainer}>
            <Text style={styles.title}>Ready to Start Coding</Text>
            <Text style={styles.subtitle}>
              Create a new OpenAgents session with your selected repository
            </Text>
          </View>

          {activeRepository && (
            <View style={styles.repositoryInfoContainer}>
              <Text style={styles.repositoryLabel}>Active Repository</Text>
              <View style={styles.repositoryCard}>
                <View style={styles.repositoryHeader}>
                  <Text style={styles.repositoryName}>
                    {activeRepository.name}
                  </Text>
                  {activeRepository.isPrivate && (
                    <View style={styles.privateBadge}>
                      <Text style={styles.privateBadgeText}>Private</Text>
                    </View>
                  )}
                </View>
                <Text style={styles.repositoryOwner}>
                  {activeRepository.owner}
                </Text>
                <Text style={styles.repositoryUrl} numberOfLines={1}>
                  {activeRepository.url}
                </Text>
              </View>
              
              <TouchableOpacity
                style={styles.changeRepositoryButton}
                onPress={handleChangeRepository}
                accessibilityRole="button"
                accessibilityLabel="Change repository"
              >
                <Text style={styles.changeRepositoryText}>Change Repository</Text>
              </TouchableOpacity>
            </View>
          )}

          <View style={styles.actionContainer}>
            <TouchableOpacity
              style={[
                styles.newSessionButton,
                !activeRepository && styles.newSessionButtonDisabled,
              ]}
              onPress={() => {
                const timestamp = new Date().toISOString();
                console.log(`🎯 [NEW_SESSION] ${timestamp} Button pressed! activeRepository:`, activeRepository);
                handleCreateSession();
              }}
              disabled={!activeRepository}
              accessibilityRole="button"
              accessibilityLabel="Create new OpenAgents session"
              accessibilityState={{ disabled: !activeRepository }}
              activeOpacity={0.8}
              hitSlop={{ top: 10, bottom: 10, left: 10, right: 10 }}
            >
              <Text style={[
                styles.newSessionButtonText,
                !activeRepository && styles.newSessionButtonTextDisabled,
              ]}>
                New Session
              </Text>
            </TouchableOpacity>

            {!activeRepository && (
              <View style={styles.helperContainer}>
                <Text style={styles.helperText}>
                  Select a repository to get started
                </Text>
                {actualOnboardingData?.step && (
                  <Text style={styles.debugText}>
                    Onboarding step: {actualOnboardingData.step}
                  </Text>
                )}
                <TouchableOpacity 
                  style={styles.selectRepositoryButton}
                  onPress={onChangeRepository}
                  accessibilityRole="button"
                  accessibilityLabel="Go back to repository selection"
                >
                  <Text style={styles.selectRepositoryText}>
                    Go Back to Repository Selection
                  </Text>
                </TouchableOpacity>
              </View>
            )}
          </View>
        </View>

        <View style={styles.footerContainer}>
          <Text style={styles.footerText}>
            Your session will be created with full access to{'\n'}
            {activeRepository ? `${activeRepository.owner}/${activeRepository.name}` : 'your selected repository'}
          </Text>
        </View>
      </View>
    </ErrorBoundary>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#000000',
  },
  centerContainer: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: 24,
    marginTop: -60, // Slight upward adjustment for better visual balance
  },
  headerContainer: {
    alignItems: 'center',
    marginBottom: 40,
  },
  title: {
    fontSize: 28,
    fontWeight: 'bold',
    color: '#ffffff',
    textAlign: 'center',
    marginBottom: 12,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  subtitle: {
    fontSize: 16,
    color: '#a1a1aa',
    textAlign: 'center',
    lineHeight: 24,
    maxWidth: 320,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  repositoryInfoContainer: {
    alignItems: 'center',
    marginBottom: 40,
    width: '100%',
    maxWidth: 360,
  },
  repositoryLabel: {
    fontSize: 14,
    color: '#71717a',
    marginBottom: 12,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  repositoryCard: {
    backgroundColor: '#18181b',
    borderRadius: 12,
    padding: 16,
    width: '100%',
    borderWidth: 1,
    borderColor: '#27272a',
    marginBottom: 16,
  },
  repositoryHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: 8,
  },
  repositoryName: {
    fontSize: 18,
    fontWeight: 'bold',
    color: '#ffffff',
    flex: 1,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  privateBadge: {
    backgroundColor: '#f59e0b',
    paddingHorizontal: 8,
    paddingVertical: 2,
    borderRadius: 4,
  },
  privateBadgeText: {
    fontSize: 10,
    fontWeight: 'bold',
    color: '#000000',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  repositoryOwner: {
    fontSize: 14,
    color: '#71717a',
    marginBottom: 4,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  repositoryUrl: {
    fontSize: 12,
    color: '#52525b',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  changeRepositoryButton: {
    paddingHorizontal: 16,
    paddingVertical: 8,
  },
  changeRepositoryText: {
    color: '#60a5fa',
    fontSize: 14,
    fontWeight: 'bold',
    textAlign: 'center',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  actionContainer: {
    alignItems: 'center',
    width: '100%',
    maxWidth: 280,
  },
  newSessionButton: {
    backgroundColor: '#22c55e',
    paddingHorizontal: 48,
    paddingVertical: 18,
    borderRadius: 12,
    width: '100%',
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: 16,
    // Enhanced visual depth and feedback
    shadowColor: '#22c55e',
    shadowOffset: {
      width: 0,
      height: 6,
    },
    shadowOpacity: 0.4,
    shadowRadius: 12,
    elevation: 10,
    // Add border for better definition
    borderWidth: 1,
    borderColor: '#16a34a',
    // Minimum touch target size
    minHeight: 56,
  },
  newSessionButtonDisabled: {
    backgroundColor: '#374151',
    shadowOpacity: 0,
    elevation: 0,
    borderColor: '#4b5563',
  },
  newSessionButtonText: {
    color: '#ffffff',
    fontSize: 20,
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  newSessionButtonTextDisabled: {
    color: '#6b7280',
  },
  helperContainer: {
    alignItems: 'center',
    gap: 12,
  },
  helperText: {
    fontSize: 14,
    color: '#71717a',
    textAlign: 'center',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  debugText: {
    fontSize: 12,
    color: '#52525b',
    textAlign: 'center',
    fontStyle: 'italic',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  selectRepositoryButton: {
    backgroundColor: 'transparent',
    borderWidth: 1,
    borderColor: '#60a5fa',
    paddingHorizontal: 20,
    paddingVertical: 12,
    borderRadius: 8,
    marginTop: 8,
  },
  selectRepositoryText: {
    color: '#60a5fa',
    fontSize: 14,
    fontWeight: 'bold',
    textAlign: 'center',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  footerContainer: {
    padding: 24,
    alignItems: 'center',
  },
  footerText: {
    fontSize: 12,
    color: '#52525b',
    textAlign: 'center',
    lineHeight: 18,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  loadingText: {
    fontSize: 16,
    color: '#a1a1aa',
    textAlign: 'center',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  errorText: {
    fontSize: 16,
    color: '#ef4444',
    textAlign: 'center',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
});