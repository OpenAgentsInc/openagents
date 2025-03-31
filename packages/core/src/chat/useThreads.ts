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
      const thread = await threadRepository.createThread({
        title: title || 'New Chat'
      });
      
      // Refresh the threads list
      await loadThreads();
      
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
      const result = await threadRepository.deleteThread(threadId);
      
      // Refresh the threads list
      if (result) {
        await loadThreads();
      }
      
      return result;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error deleting thread:', error);
      return false;
    }
  }, [loadThreads]);
  
  // Update a thread
  const updateThread = useCallback(async (threadId: string, updates: Partial<Thread>): Promise<Thread | null> => {
    try {
      const result = await threadRepository.updateThread(threadId, updates);
      
      // Refresh the threads list
      if (result) {
        await loadThreads();
      }
      
      return result;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error updating thread:', error);
      return null;
    }
  }, [loadThreads]);
  
  // Initial load
  useEffect(() => {
    loadThreads();
  }, [loadThreads]);
  
  // Set up auto-refresh if enabled
  useEffect(() => {
    if (refreshInterval > 0) {
      const intervalId = setInterval(() => {
        loadThreads();
      }, refreshInterval);
      
      return () => clearInterval(intervalId);
    }
  }, [refreshInterval, loadThreads]);
  
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