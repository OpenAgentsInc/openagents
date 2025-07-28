import React, { useEffect, useState } from 'react';
import {
  View,
  TouchableOpacity,
  StyleSheet,
  Platform,
  Alert,
  RefreshControl,
  ScrollView,
} from 'react-native';
import { useQuery, useMutation } from 'convex/react';
import { api } from '../../convex/_generated/api';
import { Text, ThinkingAnimation, ErrorBoundary } from '../index';
import { useConfectAuth } from '../../contexts/SimpleConfectAuthContext';
import { useRepositoryState } from '../../hooks/useRepositoryState';

interface Repository {
  id: number;
  name: string;
  fullName: string;
  owner: string;
  isPrivate: boolean;
  defaultBranch?: string;
  updatedAt: string;
  description?: string;
  language?: string;
  htmlUrl: string;
}

interface RepositorySelectionScreenProps {
  onRepositorySelected: (repository: Repository) => void;
  onSkip?: () => void;
}

/**
 * Repository Selection Screen for onboarding flow.
 * Shows the user's 5 most recently updated GitHub repositories.
 */
export function RepositorySelectionScreen({ 
  onRepositorySelected, 
  onSkip 
}: RepositorySelectionScreenProps) {
  const { isAuthenticated, user } = useConfectAuth();
  const [isRefreshing, setIsRefreshing] = useState(false);

  // Query cached repositories
  const repositoriesData = useQuery(
    api.confect.github.getUserRepositories,
    isAuthenticated ? {} : "skip"
  );

  // Mutation to fetch fresh repositories
  const fetchRepositories = useMutation(api.confect.github.fetchUserRepositories);
  
  // Repository state management
  const { setActiveRepository } = useRepositoryState();

  // Handle repository selection
  const handleRepositorySelect = async (repository: Repository) => {
    try {
      const timestamp = new Date().toISOString();
      console.log(`🔄 [REPO_SELECTION] ${timestamp} User selected repository:`, {
        name: repository.name,
        owner: repository.owner,
        isPrivate: repository.isPrivate,
      });

      await setActiveRepository({
        repositoryUrl: repository.htmlUrl,
        repositoryName: repository.name,
        repositoryOwner: repository.owner,
        isPrivate: repository.isPrivate,
        defaultBranch: repository.defaultBranch,
      });

      console.log(`✅ [REPO_SELECTION] ${timestamp} Repository set as active successfully`);
      onRepositorySelected(repository);
    } catch (error) {
      const timestamp = new Date().toISOString();
      console.error(`❌ [REPO_SELECTION] ${timestamp} Failed to set active repository:`, error);
      Alert.alert(
        'Error',
        'Failed to set the selected repository. Please try again.',
        [{ text: 'OK' }]
      );
    }
  };

  // Handle refresh
  const handleRefresh = async () => {
    try {
      setIsRefreshing(true);
      const timestamp = new Date().toISOString();
      console.log(`🔄 [REPO_SELECTION] ${timestamp} User triggered repository refresh`);

      await fetchRepositories({ forceRefresh: true });
      
      console.log(`✅ [REPO_SELECTION] ${timestamp} Repository refresh completed`);
    } catch (error) {
      const timestamp = new Date().toISOString();
      console.error(`❌ [REPO_SELECTION] ${timestamp} Failed to refresh repositories:`, error);
      Alert.alert(
        'Refresh Failed',
        'Unable to fetch the latest repositories. Please check your connection and try again.',
        [{ text: 'OK' }]
      );
    } finally {
      setIsRefreshing(false);
    }
  };

  // Handle skip
  const handleSkip = () => {
    const timestamp = new Date().toISOString();
    console.log(`⏭️ [REPO_SELECTION] ${timestamp} User skipped repository selection`);
    onSkip?.();
  };

  // Loading state
  if (!isAuthenticated) {
    return (
      <View style={styles.container}>
        <View style={styles.emptyState}>
          <Text style={styles.emptyStateText}>Please log in to continue</Text>
        </View>
      </View>
    );
  }

  // No data available
  if (!repositoriesData) {
    return (
      <View style={styles.container}>
        <View style={styles.header}>
          <Text style={styles.title}>Select Your Primary Repository</Text>
          <Text style={styles.subtitle}>
            Choose the repository you'll work with most often
          </Text>
        </View>

        <View style={styles.loadingContainer}>
          <ThinkingAnimation size={40} style={styles.loadingAnimation} />
          <Text style={styles.loadingText}>Loading your repositories...</Text>
        </View>

        <View style={styles.footer}>
          <TouchableOpacity style={styles.skipButton} onPress={handleSkip}>
            <Text style={styles.skipButtonText}>Skip for now</Text>
          </TouchableOpacity>
        </View>
      </View>
    );
  }

  // Error state
  if (repositoriesData.hasError && repositoriesData.repositories.length === 0) {
    return (
      <View style={styles.container}>
        <View style={styles.header}>
          <Text style={styles.title}>Select Your Primary Repository</Text>
          <Text style={styles.subtitle}>Unable to load repositories</Text>
        </View>

        <View style={styles.errorContainer}>
          <Text style={styles.errorText}>
            {repositoriesData.errorMessage || 'Failed to load repositories'}
          </Text>
          <TouchableOpacity style={styles.retryButton} onPress={handleRefresh}>
            <Text style={styles.retryButtonText}>Try Again</Text>
          </TouchableOpacity>
        </View>

        <View style={styles.footer}>
          <TouchableOpacity style={styles.skipButton} onPress={handleSkip}>
            <Text style={styles.skipButtonText}>Skip for now</Text>
          </TouchableOpacity>
        </View>
      </View>
    );
  }

  // Main content with repositories
  return (
    <ErrorBoundary
      onError={(error, errorInfo) => {
        const timestamp = new Date().toISOString();
        console.error(`❌ [REPO_SELECTION] ${timestamp} Component error:`, {
          error: error.message,
          stack: error.stack,
          componentStack: errorInfo.componentStack,
          user: user?.githubUsername,
        });
      }}
    >
      <View style={styles.container}>
        <View style={styles.header}>
          <Text style={styles.title}>Select Your Primary Repository</Text>
          <Text style={styles.subtitle}>
            Choose the repository you'll work with most often
          </Text>
          {repositoriesData.hasError && (
            <Text style={styles.warningText}>
              ⚠️ Some repositories may not be up to date
            </Text>
          )}
        </View>

        <ScrollView 
          style={styles.scrollContainer}
          refreshControl={
            <RefreshControl
              refreshing={isRefreshing}
              onRefresh={handleRefresh}
              tintColor="#ffffff"
              colors={['#ffffff']}
            />
          }
          showsVerticalScrollIndicator={false}
          testID="repository-scroll"
        >
          <View style={styles.repositoriesContainer}>
            {repositoriesData.repositories.map((repository: Repository) => (
              <RepositoryCard
                key={repository.id}
                repository={repository}
                onSelect={() => handleRepositorySelect(repository)}
              />
            ))}

            {repositoriesData.repositories.length === 0 && (
              <View style={styles.emptyRepositoriesContainer}>
                <Text style={styles.emptyRepositoriesText}>
                  No repositories found
                </Text>
                <Text style={styles.emptyRepositoriesSubtext}>
                  Create a repository on GitHub to get started
                </Text>
              </View>
            )}
          </View>
        </ScrollView>

        <View style={styles.footer}>
          <Text style={styles.footerHint}>
            You can change this later in settings
          </Text>
          <TouchableOpacity style={styles.skipButton} onPress={handleSkip}>
            <Text style={styles.skipButtonText}>Skip for now</Text>
          </TouchableOpacity>
        </View>
      </View>
    </ErrorBoundary>
  );
}

interface RepositoryCardProps {
  repository: Repository;
  onSelect: () => void;
}

function RepositoryCard({ repository, onSelect }: RepositoryCardProps) {
  const formatUpdatedAt = (dateString: string): string => {
    try {
      const date = new Date(dateString);
      const now = new Date();
      const diffInDays = Math.floor((now.getTime() - date.getTime()) / (1000 * 60 * 60 * 24));
      
      if (diffInDays === 0) return 'Updated today';
      if (diffInDays === 1) return 'Updated yesterday';
      if (diffInDays < 7) return `Updated ${diffInDays} days ago`;
      if (diffInDays < 30) return `Updated ${Math.floor(diffInDays / 7)} weeks ago`;
      if (diffInDays < 365) return `Updated ${Math.floor(diffInDays / 30)} months ago`;
      return `Updated ${Math.floor(diffInDays / 365)} years ago`;
    } catch (error) {
      return 'Recently updated';
    }
  };

  return (
    <TouchableOpacity
      style={styles.repositoryCard}
      onPress={onSelect}
      activeOpacity={0.7}
      accessibilityRole="button"
      accessibilityLabel={`Select ${repository.name} repository`}
    >
      <View style={styles.repositoryHeader}>
        <View style={styles.repositoryNameContainer}>
          <Text style={styles.repositoryName} numberOfLines={1}>
            {repository.name}
          </Text>
          <View style={styles.repositoryBadgeContainer}>
            {repository.isPrivate && (
              <View style={styles.privateBadge}>
                <Text style={styles.privateBadgeText}>Private</Text>
              </View>
            )}
          </View>
        </View>
        <Text style={styles.repositoryOwner} numberOfLines={1}>
          {repository.owner}
        </Text>
      </View>

      <View style={styles.repositoryDetails}>
        <Text style={styles.repositoryDescription} numberOfLines={2}>
          {repository.description || 'No description available'}
        </Text>
        
        <View style={styles.repositoryMeta}>
          {repository.language && (
            <View style={styles.languageContainer}>
              <View style={[styles.languageDot, { backgroundColor: getLanguageColor(repository.language) }]} />
              <Text style={styles.languageText}>{repository.language}</Text>
            </View>
          )}
          <Text style={styles.updatedText}>
            {formatUpdatedAt(repository.updatedAt)}
          </Text>
        </View>
      </View>
    </TouchableOpacity>
  );
}

// Helper function to get language colors (simplified version)
function getLanguageColor(language: string): string {
  const colors: Record<string, string> = {
    'TypeScript': '#3178c6',
    'JavaScript': '#f1e05a',
    'Python': '#3572A5',
    'Java': '#b07219',
    'Go': '#00ADD8',
    'Rust': '#dea584',
    'Swift': '#fa7343',
    'Kotlin': '#A97BFF',
    'C++': '#f34b7d',
    'C#': '#239120',
    'PHP': '#4F5D95',
    'Ruby': '#701516',
    'HTML': '#e34c26',
    'CSS': '#1572B6',
  };
  return colors[language] || '#6b7280';
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#000000',
  },
  header: {
    padding: 24,
    paddingBottom: 16,
  },
  title: {
    fontSize: 24,
    fontWeight: 'bold',
    color: '#ffffff',
    marginBottom: 8,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  subtitle: {
    fontSize: 16,
    color: '#a1a1aa',
    lineHeight: 22,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  warningText: {
    fontSize: 14,
    color: '#f59e0b',
    marginTop: 8,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  scrollContainer: {
    flex: 1,
  },
  repositoriesContainer: {
    padding: 16,
  },
  repositoryCard: {
    backgroundColor: '#18181b',
    borderRadius: 12,
    padding: 16,
    marginBottom: 12,
    borderWidth: 1,
    borderColor: '#27272a',
  },
  repositoryHeader: {
    marginBottom: 12,
  },
  repositoryNameContainer: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: 4,
  },
  repositoryName: {
    fontSize: 18,
    fontWeight: 'bold',
    color: '#ffffff',
    flex: 1,
    marginRight: 8,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  repositoryBadgeContainer: {
    flexDirection: 'row',
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
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  repositoryDetails: {
    gap: 8,
  },
  repositoryDescription: {
    fontSize: 14,
    color: '#a1a1aa',
    lineHeight: 20,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  repositoryMeta: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
  },
  languageContainer: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  languageDot: {
    width: 8,
    height: 8,
    borderRadius: 4,
    marginRight: 6,
  },
  languageText: {
    fontSize: 12,
    color: '#71717a',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  updatedText: {
    fontSize: 12,
    color: '#71717a',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  emptyState: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: 40,
  },
  emptyStateText: {
    fontSize: 16,
    color: '#a1a1aa',
    textAlign: 'center',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  loadingContainer: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: 40,
  },
  loadingAnimation: {
    marginBottom: 16,
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
  errorContainer: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: 40,
  },
  errorText: {
    fontSize: 16,
    color: '#ef4444',
    textAlign: 'center',
    marginBottom: 24,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  retryButton: {
    backgroundColor: '#000000',
    borderWidth: 1,
    borderColor: '#ffffff',
    paddingHorizontal: 24,
    paddingVertical: 12,
    borderRadius: 8,
  },
  retryButtonText: {
    color: '#ffffff',
    fontSize: 14,
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  emptyRepositoriesContainer: {
    padding: 40,
    alignItems: 'center',
  },
  emptyRepositoriesText: {
    fontSize: 16,
    color: '#a1a1aa',
    textAlign: 'center',
    marginBottom: 8,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  emptyRepositoriesSubtext: {
    fontSize: 14,
    color: '#71717a',
    textAlign: 'center',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  footer: {
    padding: 24,
    paddingTop: 16,
    alignItems: 'center',
  },
  footerHint: {
    fontSize: 12,
    color: '#71717a',
    textAlign: 'center',
    marginBottom: 16,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
  skipButton: {
    paddingHorizontal: 24,
    paddingVertical: 12,
  },
  skipButtonText: {
    color: '#71717a',
    fontSize: 14,
    fontWeight: 'bold',
    textAlign: 'center',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace',
    } as const),
  },
});