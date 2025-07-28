import { useState, useEffect } from 'react';
import { useQuery, useMutation } from 'convex/react';
import { api } from '../convex/_generated/api';
import { useConfectAuth } from '../contexts/SimpleConfectAuthContext';

interface Repository {
  url: string;
  name: string;
  owner: string;
  isPrivate: boolean;
  defaultBranch?: string;
}

export interface RepositoryState {
  activeRepository: Repository | null;
  isLoadingRepository: boolean;
  repositoryError: string | null;
  setActiveRepository: (repository: {
    repositoryUrl: string;
    repositoryName: string;
    repositoryOwner: string;
    isPrivate: boolean;
    defaultBranch?: string;
  }) => Promise<void>;
  refreshActiveRepository: () => Promise<void>;
}

/**
 * Hook for managing repository state within ConvexProvider context.
 * Must be used inside a component wrapped by ConvexProvider.
 */
export function useRepositoryState(): RepositoryState {
  const { isAuthenticated } = useConfectAuth();
  const [activeRepository, setActiveRepositoryState] = useState<Repository | null>(null);
  const [isLoadingRepository, setIsLoadingRepository] = useState(false);
  const [repositoryError, setRepositoryError] = useState<string | null>(null);

  // Query onboarding progress to get active repository
  const onboardingProgress = useQuery(
    api.confect.onboarding.getOnboardingProgress,
    isAuthenticated ? {} : "skip"
  );

  // Mutation to set active repository
  const setActiveRepositoryMutation = useMutation(api.confect.onboarding.setActiveRepository);

  // Update active repository when onboarding progress changes
  useEffect(() => {
    if (onboardingProgress && onboardingProgress.activeRepository) {
      setActiveRepositoryState(onboardingProgress.activeRepository);
      setRepositoryError(null);
    } else if (onboardingProgress && !onboardingProgress.activeRepository) {
      setActiveRepositoryState(null);
    }
  }, [onboardingProgress]);

  const setActiveRepository = async (repository: {
    repositoryUrl: string;
    repositoryName: string;
    repositoryOwner: string;
    isPrivate: boolean;
    defaultBranch?: string;
  }) => {
    try {
      setIsLoadingRepository(true);
      setRepositoryError(null);
      
      console.log('🔄 [REPOSITORY_STATE] Setting active repository:', {
        name: repository.repositoryName,
        owner: repository.repositoryOwner,
      });

      await setActiveRepositoryMutation(repository);

      console.log('✅ [REPOSITORY_STATE] Active repository set successfully');
    } catch (error) {
      console.error('❌ [REPOSITORY_STATE] Failed to set active repository:', error);
      setRepositoryError(error instanceof Error ? error.message : 'Failed to set repository');
      throw error;
    } finally {
      setIsLoadingRepository(false);
    }
  };

  const refreshActiveRepository = async () => {
    console.log('🔄 [REPOSITORY_STATE] Refreshing active repository state');
    // The repository state will be automatically updated via the useEffect
    // when the onboarding progress query refetches
  };

  return {
    activeRepository,
    isLoadingRepository,
    repositoryError,
    setActiveRepository,
    refreshActiveRepository,
  };
}