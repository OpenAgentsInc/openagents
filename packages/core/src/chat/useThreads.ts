import { useState, useEffect, useCallback } from 'react';
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
  
  // Load threads
  const loadThreads = useCallback(async () => {
    try {
      setIsLoading(true);
      setError(null);
      
      const threadList = await threadRepository.getAllThreads();
      setThreads(threadList);
      
      return threadList;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      setError(error);
      console.error('Error loading threads:', error);
      return [];
    } finally {
      setIsLoading(false);
    }
  }, []);
  
  // Create a new thread
  const createThread = useCallback(async (title?: string): Promise<Thread> => {
    try {
      // Create the thread directly in the database without temporary thread
      const thread = await threadRepository.createThread({
        title: title || 'New Chat'
      });
      
      // Update local state immediately for optimistic UI
      setThreads(currentThreads => [thread, ...currentThreads]);
      
      // Skip the full loadThreads to avoid showing duplicates
      
      return thread;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error creating thread:', error);
      throw error;
    }
  }, [loadThreads]);
  
  // Delete a thread
  const deleteThread = useCallback(async (threadId: string): Promise<boolean> => {
    try {
      // First optimistically update local state for immediate UI response
      setThreads(currentThreads => currentThreads.filter(thread => thread.id !== threadId));
      
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
      setThreads(currentThreads => 
        currentThreads.map(thread => 
          thread.id === threadId 
            ? { ...thread, ...updates, updatedAt: Date.now() } 
            : thread
        )
      );
      
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
  
  // Initial load with isLoading state management
  useEffect(() => {
    // Set loading state only if threads array is empty
    if (threads.length === 0) {
      setIsLoading(true);
    }
    
    loadThreads().finally(() => {
      // Clear loading state after load completes
      setIsLoading(false);
    });
  }, [loadThreads, threads.length]);
  
  // Set up auto-refresh if enabled - don't show loading state for background refreshes
  useEffect(() => {
    if (refreshInterval > 0) {
      const intervalId = setInterval(() => {
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