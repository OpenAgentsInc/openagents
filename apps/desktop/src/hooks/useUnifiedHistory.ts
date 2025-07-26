import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAuth } from '@/contexts/AuthContext';

export interface UnifiedSession {
  id: string;
  title: string;
  timestamp: string; // ISO string
  project_path?: string;
  working_directory?: string;
  first_message?: string;
  message_count?: number;
  summary?: string;
  source: 'local' | 'convex';
  file_path?: string; // Only for local sessions
  status?: string;    // Only for Convex sessions
  created_by?: string; // Only for Convex sessions
}

interface CommandResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export const useUnifiedHistory = (limit: number = 50) => {
  const [sessions, setSessions] = useState<UnifiedSession[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { user, isAuthenticated } = useAuth();

  const loadHistory = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      // Get user ID if authenticated (we can derive from GitHub ID)
      const userId = isAuthenticated && user ? user.id : undefined;

      console.log('📚 [UNIFIED-HISTORY] Loading unified session history:', {
        limit,
        userId: userId || 'MISSING',
        isAuthenticated
      });
      
      console.log('📚 [UNIFIED-HISTORY] About to call get_unified_history with:', {
        limit,
        user_id: userId
      });

      const result = await invoke<CommandResult<UnifiedSession[]>>('get_unified_history', {
        limit,
        user_id: userId,
      });

      if (result.success && result.data) {
        setSessions(result.data);
        console.log('✅ [UNIFIED-HISTORY] Loaded', result.data.length, 'unified sessions');
        
        // Log distribution by source
        const localCount = result.data.filter(s => s.source === 'local').length;
        const convexCount = result.data.filter(s => s.source === 'convex').length;
        console.log('📊 [UNIFIED-HISTORY] Distribution:', { local: localCount, convex: convexCount });
      } else {
        const errorMsg = result.error || 'Failed to load unified history';
        setError(errorMsg);
        console.error('❌ [UNIFIED-HISTORY] Error:', errorMsg);
      }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(errorMsg);
      console.error('❌ [UNIFIED-HISTORY] Exception:', errorMsg);
    } finally {
      setIsLoading(false);
    }
  }, [limit, user, isAuthenticated]);

  const refreshHistory = useCallback(() => {
    loadHistory();
  }, [loadHistory]);

  useEffect(() => {
    loadHistory();
  }, [loadHistory]);

  return {
    sessions,
    isLoading,
    error,
    refreshHistory,
  };
};