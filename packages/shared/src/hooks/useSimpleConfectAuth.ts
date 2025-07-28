import { useState, useEffect, useCallback } from 'react';

interface User {
  id: string;
  email: string;
  name?: string;
  avatar?: string;
  githubId: string;
  githubUsername: string;
}

interface UseSimpleConfectAuthConfig {
  convexUrl?: string;
  enableRealTimeSync?: boolean;
  debugMode?: boolean;
  authUrl?: string;
  clientId?: string;
  redirectUri?: string;
  scopes?: string[];
}

interface UseSimpleConfectAuthReturn {
  // Auth state
  user: User | null;
  token: string | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  error: string | null;
  
  // Auth actions
  login: () => Promise<void>;
  logout: () => Promise<void>;
  clearError: () => void;
  
  // Placeholder Confect features (to be implemented gradually)
  syncToBackend: () => Promise<void>;
  getUserStats: (includeDeviceBreakdown?: boolean) => Promise<any>;
  requestDesktopSession: (projectPath: string, initialMessage?: string, title?: string) => Promise<string>;
}

/**
 * Simplified Confect auth hook that works without complex Effect-TS patterns.
 * This is a stepping stone toward the full Effect-TS implementation.
 */
export function useSimpleConfectAuth(config: UseSimpleConfectAuthConfig = {}): UseSimpleConfectAuthReturn {
  const [user, setUser] = useState<User | null>(null);
  const [token, setToken] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const isAuthenticated = !!user && !!token;

  // Initialize auth state
  useEffect(() => {
    checkAuthState();
  }, []);

  const checkAuthState = async () => {
    try {
      // For now, return empty state - this will be enhanced gradually
      console.log('🔄 [SIMPLE_CONFECT_AUTH] Checking auth state...');
      setIsLoading(false);
    } catch (error) {
      console.error('❌ [SIMPLE_CONFECT_AUTH] Failed to check auth state:', error);
      setError(error instanceof Error ? error.message : 'Unknown error');
      setIsLoading(false);
    }
  };

  const login = useCallback(async () => {
    try {
      setIsLoading(true);
      setError(null);
      console.log('🔄 [SIMPLE_CONFECT_AUTH] Login not yet implemented');
      // Placeholder - will be implemented gradually
      throw new Error('Login not yet implemented');
    } catch (error) {
      console.error('❌ [SIMPLE_CONFECT_AUTH] Login error:', error);
      setError(error instanceof Error ? error.message : 'Login failed');
    } finally {
      setIsLoading(false);
    }
  }, []);

  const logout = useCallback(async () => {
    try {
      setIsLoading(true);
      console.log('🔄 [SIMPLE_CONFECT_AUTH] Logout');
      setUser(null);
      setToken(null);
      setError(null);
    } catch (error) {
      console.error('❌ [SIMPLE_CONFECT_AUTH] Logout error:', error);
      setError(error instanceof Error ? error.message : 'Logout failed');
    } finally {
      setIsLoading(false);
    }
  }, []);

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  const syncToBackend = useCallback(async () => {
    console.log('🔄 [SIMPLE_CONFECT_AUTH] syncToBackend placeholder');
    return Promise.resolve();
  }, []);

  const getUserStats = useCallback(async (includeDeviceBreakdown?: boolean) => {
    console.log('🔄 [SIMPLE_CONFECT_AUTH] getUserStats placeholder');
    return Promise.resolve({});
  }, []);

  const requestDesktopSession = useCallback(async (
    projectPath: string, 
    initialMessage?: string, 
    title?: string
  ): Promise<string> => {
    console.log('🔄 [SIMPLE_CONFECT_AUTH] requestDesktopSession placeholder');
    return Promise.resolve('mock-session-id');
  }, []);

  return {
    user,
    token,
    isAuthenticated,
    isLoading,
    error,
    login,
    logout,
    clearError,
    syncToBackend,
    getUserStats,
    requestDesktopSession,
  };
}