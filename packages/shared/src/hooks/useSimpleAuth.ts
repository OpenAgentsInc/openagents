import { useState, useEffect, useCallback, useRef } from 'react';
import { Effect, Runtime, Ref } from 'effect';
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

interface UseSimpleAuthConfig {
  authUrl?: string;
  clientId?: string;
  redirectUri?: string;
  scopes?: string[];
}

interface UseSimpleAuthReturn {
  // Auth state
  user: AuthUser | null;
  token: string | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  error: string | null;
  
  // Auth actions
  login: () => Promise<void>;
  logout: () => Promise<void>;
  clearError: () => void;
}

/**
 * Simplified React hook for authentication using Effect-TS patterns.
 * 
 * Provides the same API as the original AuthContext while using
 * Effect patterns for OAuth flows, token management, and error handling.
 */
export function useSimpleAuth(config: UseSimpleAuthConfig = {}): UseSimpleAuthReturn {
  const [authState, setAuthState] = useState<AuthState>({
    user: null,
    token: null,
    isLoading: true,
    isAuthenticated: false,
  });
  const [error, setError] = useState<string | null>(null);
  
  const authStateRef = useRef<Ref.Ref<AuthState> | null>(null);
  const authConfig = useRef<AuthConfig>({ ...getDefaultAuthConfig(), ...config });
  const initializingRef = useRef(false);
  
  // Initialize auth service
  useEffect(() => {
    if (initializingRef.current) return;
    initializingRef.current = true;
    const initializeAuth = async () => {
      try {
        setError(null);
        
        // Initialize auth state
        const initProgram = Effect.gen(function* () {
          const authStateRefValue = yield* createAuthState();
          
          // Check for stored authentication
          const storedAuth = yield* checkStoredAuth();
          yield* updateAuthState(authStateRefValue, storedAuth);
          
          return { authStateRefValue, storedAuth };
        });
        
        const { authStateRefValue, storedAuth } = await Runtime.runPromise(Runtime.defaultRuntime)(initProgram);
        
        authStateRef.current = authStateRefValue;
        setAuthState(storedAuth);
        
      } catch (err) {
        console.error('Failed to initialize Auth service:', err);
        setError(String(err));
        setAuthState(prev => ({ ...prev, isLoading: false }));
      }
    };
    
    initializeAuth();
    
    // Cleanup on unmount
    return () => {
      initializingRef.current = false;
    };
  }, []);
  
  // Login function
  const loginFn = useCallback(async () => {
    if (!authStateRef.current) {
      throw new Error('Auth service not initialized');
    }
    
    try {
      setError(null);
      setAuthState(prev => ({ ...prev, isLoading: true }));
      
      const newAuthState = await Runtime.runPromise(Runtime.defaultRuntime)(
        login(authConfig.current).pipe(
          Effect.tap(authState => 
            updateAuthState(authStateRef.current!, authState)
          )
        )
      );
      
      setAuthState(newAuthState);
      
    } catch (err) {
      console.error('❌ [AUTH] Login failed:', err);
      
      const errorMessage = err instanceof Error ? err.message : 'Login failed. Please try again.';
      setError(errorMessage);
      
      setAuthState(prev => ({ ...prev, isLoading: false }));
      throw err;
    }
  }, []);
  
  // Logout function
  const logoutFn = useCallback(async () => {
    if (!authStateRef.current) {
      return;
    }
    
    try {
      setError(null);
      setAuthState(prev => ({ ...prev, isLoading: true }));
      
      const newAuthState = await Runtime.runPromise(Runtime.defaultRuntime)(
        logout().pipe(
          Effect.tap(authState => 
            updateAuthState(authStateRef.current!, authState)
          )
        )
      );
      
      setAuthState(newAuthState);
      
    } catch (err) {
      console.error('❌ [AUTH] Logout failed:', err);
      setError('Logout failed. Please try again.');
      setAuthState(prev => ({ ...prev, isLoading: false }));
    }
  }, []);
  
  // Clear error function
  const clearError = useCallback(() => {
    setError(null);
  }, []);
  
  return {
    // Auth state
    user: authState.user,
    token: authState.token,
    isAuthenticated: authState.isAuthenticated,
    isLoading: authState.isLoading,
    error,
    
    // Auth actions
    login: loginFn,
    logout: logoutFn,
    clearError,
  };
}