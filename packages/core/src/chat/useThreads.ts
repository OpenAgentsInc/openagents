import { useState, useEffect, useCallback, useRef } from 'react';
import { threadRepository } from '../db/repositories';
import { Thread } from '../db/types';

/**
 * Hook options
 */
export interface UseThreadsOptions {
  /**
   * Auto-refresh interval in milliseconds
   * Default: 0 (no auto-refresh)
   */
  refreshInterval?: number;
}

/**
 * Deep equality comparison function
 */
function areThreadListsEqual(oldThreads: Thread[], newThreads: Thread[]): boolean {
  if (oldThreads.length !== newThreads.length) {
    return false;
  }

  for (let i = 0; i < oldThreads.length; i++) {
    const oldThread = oldThreads[i];
    const newThread = newThreads[i];

    // Quick ID check
    if (oldThread.id !== newThread.id) {
      return false;
    }

    // Check essential properties
    if (
      oldThread.title !== newThread.title ||
      oldThread.updatedAt !== newThread.updatedAt ||
      oldThread.modelId !== newThread.modelId ||
      oldThread.systemPrompt !== newThread.systemPrompt
    ) {
      return false;
    }
  }

  return true;
}

/**
 * Hook for accessing thread data
 */
export function useThreads(options: UseThreadsOptions = {}) {
  const { refreshInterval = 0 } = options;

  // Thread list state
  const [threads, setThreads] = useState<Thread[]>([]);

  // Loading state
  const [isLoading, setIsLoading] = useState<boolean>(true);

  // Error state
  const [error, setError] = useState<Error | null>(null);

  // Ref to store the last threads list for comparison
  const lastThreadsRef = useRef<Thread[]>([]);

  // Track if component is mounted
  const isMountedRef = useRef<boolean>(true);

  // Load threads
  const loadThreads = useCallback(async () => {
    try {
      if (!isMountedRef.current) return [];

      // Only set loading on first load, not refreshes
      if (threads.length === 0) {
        setIsLoading(true);
      }

      setError(null);

      const threadList = await threadRepository.getAllThreads();

      // Only update state if threads actually changed (deep comparison)
      if (!areThreadListsEqual(lastThreadsRef.current, threadList)) {
        if (isMountedRef.current) {
          // console.log('Updating threads state - change detected');
          setThreads(threadList);
          lastThreadsRef.current = threadList;
        }
      }

      return threadList;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      if (isMountedRef.current) {
        setError(error);
      }
      console.error('Error loading threads:', error);
      return [];
    } finally {
      if (isMountedRef.current) {
        setIsLoading(false);
      }
    }
  }, [threads.length]);

  // Create a new thread
  const createThread = useCallback(async (title?: string): Promise<Thread> => {
    try {
      // Create the thread directly in the database without temporary thread
      const thread = await threadRepository.createThread({
        title: title || 'New Chat'
      });

      // Update local state immediately for optimistic UI
      if (isMountedRef.current) {
        setThreads(currentThreads => {
          const newThreads = [thread, ...currentThreads];
          lastThreadsRef.current = newThreads;
          return newThreads;
        });
      }

      return thread;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error creating thread:', error);
      throw error;
    }
  }, []);

  // Delete a thread
  const deleteThread = useCallback(async (threadId: string): Promise<boolean> => {
    try {
      // First optimistically update local state for immediate UI response
      if (isMountedRef.current) {
        setThreads(currentThreads => {
          const newThreads = currentThreads.filter(thread => thread.id !== threadId);
          lastThreadsRef.current = newThreads;
          return newThreads;
        });
      }

      // Then actually perform the deletion
      const result = await threadRepository.deleteThread(threadId);

      // Refresh the threads list only after backend confirms success
      if (result) {
        await loadThreads();
      } else {
        // If deletion failed, refresh to restore the thread in UI
        await loadThreads();
      }

      return result;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error deleting thread:', error);
      // On error, refresh to restore correct state
      await loadThreads();
      return false;
    }
  }, [loadThreads]);

  // Update a thread
  const updateThread = useCallback(async (threadId: string, updates: Partial<Thread>): Promise<Thread | null> => {
    try {
      // First optimistically update local state for immediate UI response
      if (isMountedRef.current) {
        setThreads(currentThreads => {
          const newThreads = currentThreads.map(thread =>
            thread.id === threadId
              ? { ...thread, ...updates, updatedAt: Date.now() }
              : thread
          );
          lastThreadsRef.current = newThreads;
          return newThreads;
        });
      }

      // Then actually perform the update
      const result = await threadRepository.updateThread(threadId, updates);

      // Refresh the threads list only after backend confirms success
      if (result) {
        await loadThreads();
      } else {
        // If update failed, refresh to restore the thread in UI
        await loadThreads();
      }

      return result;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error updating thread:', error);
      // On error, refresh to restore correct state
      await loadThreads();
      return null;
    }
  }, [loadThreads]);

  // Initial load when component mounts
  useEffect(() => {
    // Reset the mounted ref
    isMountedRef.current = true;

    // Initial load
    loadThreads();

    // Cleanup function to prevent memory leaks
    return () => {
      isMountedRef.current = false;
    };
  }, [loadThreads]);

  // Set up auto-refresh if enabled - don't show loading state for background refreshes
  useEffect(() => {
    if (refreshInterval > 0) {
      const intervalId = setInterval(() => {
        // Skip if component unmounted
        if (!isMountedRef.current) return;

        // Run the load without updating the loading state to prevent flashing
        loadThreads().catch(err => {
          console.error('Error in background refresh:', err);
        });
      }, refreshInterval);

      return () => clearInterval(intervalId);
    }
  }, [refreshInterval, loadThreads]);

  // Set up a one-time refresh when a thread is created/deleted - without loading indicator
  useEffect(() => {
    const handleThreadChange = () => {
      // Skip if component unmounted
      if (!isMountedRef.current) return;

      // Run the load without updating the loading state to prevent flashing
      loadThreads().catch(err => {
        console.error('Error in event-triggered refresh:', err);
      });
    };

    // Listen for thread changes
    window.addEventListener('thread-changed', handleThreadChange);

    return () => {
      window.removeEventListener('thread-changed', handleThreadChange);
    };
  }, [loadThreads]);

  return {
    threads,
    isLoading,
    error,
    refresh: loadThreads,
    createThread,
    deleteThread,
    updateThread
  };
}
