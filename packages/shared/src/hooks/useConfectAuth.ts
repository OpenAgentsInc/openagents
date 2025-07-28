import { useState, useEffect, useCallback, useRef } from 'react';
import { Effect, Runtime, Ref, Option } from 'effect';
import { 
  AuthState,
  AuthUser,
  AuthConfig,
  checkStoredAuth,
  login,
  logout,
  createAuthState,
  updateAuthState,
  getDefaultAuthConfig
} from '../services/SimpleAuthService';
// Import types only to avoid runtime dependency issues
interface ConfectIntegrationConfig {
  convexUrl: string;
  enableRealTimeSync?: boolean;
  debugMode?: boolean;
}

interface UseConfectAuthConfig extends UseSimpleAuthConfig {
  convexUrl: string;
  enableRealTimeSync?: boolean;
  debugMode?: boolean;
}

interface UseSimpleAuthConfig {
  authUrl?: string;
  clientId?: string;
  redirectUri?: string;
  scopes?: string[];
}

interface UseConfectAuthReturn {
  // Auth state (same as useSimpleAuth)
  user: AuthUser | null;
  token: string | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  error: string | null;
  
  // Auth actions (enhanced with Confect)
  login: () => Promise<void>;
  logout: () => Promise<void>;
  clearError: () => void;
  
  // Confect-specific features
  syncToBackend: () => Promise<void>;
  getUserStats: (includeDeviceBreakdown?: boolean) => Promise<any>;
  requestDesktopSession: (projectPath: string, initialMessage?: string, title?: string) => Promise<string>;
}

/**
 * Enhanced React hook for authentication using Effect-TS patterns with Confect integration.
 * 
 * Extends useSimpleAuth with automatic backend synchronization and database operations
 * through Confect Effect-powered functions.
 */
export function useConfectAuth(config: UseConfectAuthConfig): UseConfectAuthReturn {
  const [authState, setAuthState] = useState<AuthState>({
    user: null,
    token: null,
    isLoading: true,
    isAuthenticated: false,
  });
  const [error, setError] = useState<string | null>(null);
  
  const authStateRef = useRef<Ref.Ref<AuthState> | null>(null);
  const confectServicesRef = useRef<any>(null);
  const authConfig = useRef<AuthConfig>({ ...getDefaultAuthConfig(), ...config });
  const initializingRef = useRef(false);
  
  // Initialize auth service with Confect integration
  useEffect(() => {
    if (initializingRef.current) return;
    initializingRef.current = true;
    
    const initializeAuth = async () => {
      try {
        setError(null);
        
        // Initialize both auth state and Confect services
        const initProgram = Effect.gen(function* () {
          // Create auth state ref
          const authStateRefValue = yield* createAuthState();
          
          // Initialize Confect integration (handle import gracefully)
          const confectIntegration = yield* Effect.tryPromise({
            try: () => import('../../../convex/confect/integration'),
            catch: (error) => new Error(`Failed to load Confect integration: ${error}`)
          });
          
          const confectServices = yield* confectIntegration.createIntegratedEffectServices({
            convexUrl: config.convexUrl,
            enableRealTimeSync: config.enableRealTimeSync,
            debugMode: config.debugMode,
          });
          
          // Check for stored authentication
          const storedAuth = yield* checkStoredAuth();
          yield* updateAuthState(authStateRefValue, storedAuth);
          
          // If user is authenticated, sync to backend
          if (storedAuth.isAuthenticated && storedAuth.user && storedAuth.token) {
            yield* confectServices.syncUser(storedAuth.user, storedAuth.token).pipe(
              Effect.catchAll(error => 
                Effect.log(`Failed to sync user to backend: ${error}`)
              )
            );
          }
          
          return { authStateRefValue, confectServices, storedAuth };
        });
        
        const { authStateRefValue, confectServices, storedAuth } = await Runtime.runPromise(Runtime.defaultRuntime)(initProgram);
        
        authStateRef.current = authStateRefValue;
        confectServicesRef.current = confectServices;
        setAuthState(storedAuth);
        
      } catch (err) {
        console.error('Failed to initialize Confect Auth service:', err);
        setError(String(err));
        setAuthState(prev => ({ ...prev, isLoading: false }));
      }
    };
    
    initializeAuth();
    
    // Cleanup on unmount
    return () => {
      initializingRef.current = false;
    };
  }, [config.convexUrl, config.enableRealTimeSync, config.debugMode]);
  
  // Enhanced login function with backend sync
  const loginFn = useCallback(async () => {
    if (!authStateRef.current || !confectServicesRef.current) {
      throw new Error('Auth service not initialized');
    }
    
    try {
      setError(null);
      setAuthState(prev => ({ ...prev, isLoading: true }));
      
      const loginProgram = Effect.gen(function* () {
        // Perform login
        const newAuthState = yield* login(authConfig.current);
        
        // Update local state
        yield* updateAuthState(authStateRef.current!, newAuthState);
        
        // Sync user to backend
        if (newAuthState.isAuthenticated && newAuthState.user && newAuthState.token) {
          yield* confectServicesRef.current.syncUser(newAuthState.user, newAuthState.token);
          yield* Effect.log(`✅ [CONFECT-AUTH] User synced to backend: ${newAuthState.user.githubUsername}`);
        }
        
        return newAuthState;
      });
      
      const newAuthState = await Runtime.runPromise(Runtime.defaultRuntime)(loginProgram);
      setAuthState(newAuthState);
      
    } catch (err) {
      console.error('❌ [CONFECT-AUTH] Login failed:', err);
      
      const errorMessage = err instanceof Error ? err.message : 'Login failed. Please try again.';
      setError(errorMessage);
      
      setAuthState(prev => ({ ...prev, isLoading: false }));
      throw err;
    }
  }, []);
  
  // Enhanced logout function with backend cleanup
  const logoutFn = useCallback(async () => {
    if (!authStateRef.current) {
      return;
    }
    
    try {
      setError(null);
      setAuthState(prev => ({ ...prev, isLoading: true }));
      
      const logoutProgram = Effect.gen(function* () {
        // Perform logout
        const newAuthState = yield* logout();
        
        // Update local state
        yield* updateAuthState(authStateRef.current!, newAuthState);
        
        yield* Effect.log(`✅ [CONFECT-AUTH] Logout completed`);
        
        return newAuthState;
      });
      
      const newAuthState = await Runtime.runPromise(Runtime.defaultRuntime)(logoutProgram);
      setAuthState(newAuthState);
      
    } catch (err) {
      console.error('❌ [CONFECT-AUTH] Logout failed:', err);
      setError('Logout failed. Please try again.');
      setAuthState(prev => ({ ...prev, isLoading: false }));
    }
  }, []);
  
  // Sync current user to backend
  const syncToBackend = useCallback(async () => {
    if (!confectServicesRef.current) {
      throw new Error('Confect services not initialized');
    }
    
    if (!authState.isAuthenticated || !authState.user || !authState.token) {
      throw new Error('User not authenticated');
    }
    
    await Runtime.runPromise(Runtime.defaultRuntime)(
      confectServicesRef.current.syncUser(authState.user, authState.token)
    );
  }, [authState]);
  
  // Get user APM stats from backend
  const getUserStats = useCallback(async (includeDeviceBreakdown: boolean = false) => {
    if (!confectServicesRef.current) {
      throw new Error('Confect services not initialized');
    }
    
    return await Runtime.runPromise(Runtime.defaultRuntime)(
      confectServicesRef.current.getUserAPMStats(includeDeviceBreakdown)
    );
  }, []);
  
  // Request desktop session via Confect
  const requestDesktopSession = useCallback(async (
    projectPath: string, 
    initialMessage?: string, 
    title?: string
  ) => {
    if (!confectServicesRef.current) {
      throw new Error('Confect services not initialized');
    }
    
    return await Runtime.runPromise(Runtime.defaultRuntime)(
      confectServicesRef.current.requestDesktopSession(projectPath, initialMessage, title)
    );
  }, []);
  
  // Clear error function
  const clearError = useCallback(() => {
    setError(null);
  }, []);
  
  return {
    // Auth state (same as useSimpleAuth)
    user: authState.user,
    token: authState.token,
    isAuthenticated: authState.isAuthenticated,
    isLoading: authState.isLoading,
    error,
    
    // Auth actions (enhanced with Confect)
    login: loginFn,
    logout: logoutFn,
    clearError,
    
    // Confect-specific features
    syncToBackend,
    getUserStats,
    requestDesktopSession,
  };
}