import { Effect, Data, Ref, Schedule, Duration } from "effect";
import { getStorageValue, setStorageValue, removeStorageValue } from "./SimpleStorageService";
import { isReactNative } from '../utils/platform';

// Tagged error types for auth operations
export class AuthError extends Data.TaggedError("AuthError")<{
  operation: string;
  message: string;
  cause?: unknown;
}> {}

export class AuthNetworkError extends Data.TaggedError("AuthNetworkError")<{
  operation: string;
  status?: number;
  message: string;
  retryable: boolean;
  cause?: unknown;
}> {}

// Auth data types
export interface AuthUser {
  id: string;
  email: string;
  name?: string;
  avatar?: string;
  githubId: string;
  githubUsername: string;
}

export interface AuthState {
  user: AuthUser | null;
  token: string | null;
  isLoading: boolean;
  isAuthenticated: boolean;
}

export interface AuthConfig {
  authUrl: string;
  clientId: string;
  redirectUri: string;
  scopes: string[];
}

// Storage keys
const TOKEN_KEY = "openauth_token";
const USER_KEY = "openauth_user";

// Default auth configuration
export const getDefaultAuthConfig = (): AuthConfig => ({
  authUrl: (typeof process !== 'undefined' && process.env?.NEXT_PUBLIC_OPENAUTH_URL) || 
           (typeof process !== 'undefined' && process.env?.EXPO_PUBLIC_OPENAUTH_URL) || 
           'https://auth.openagents.com',
  clientId: 'openagents-app',
  redirectUri: isReactNative() 
    ? 'openagents://auth/callback' 
    : 'openagents://auth/callback',
  scopes: ['openid', 'profile', 'email'],
});

// Auth service functions
export const checkStoredAuth = () =>
  Effect.gen(function* () {
    yield* Effect.log("🔐 [AUTH] Checking stored authentication...");
    
    const storedToken = yield* getStorageValue(TOKEN_KEY).pipe(
      Effect.catchAll(_ => Effect.succeed(null))
    );
    
    const storedUserJson = yield* getStorageValue(USER_KEY).pipe(
      Effect.catchAll(_ => Effect.succeed(null))
    );
    
    if (storedToken && storedUserJson) {
      const user = JSON.parse(storedUserJson) as AuthUser;
      yield* Effect.log(`✅ [AUTH] Restored authentication for: ${user.githubUsername}`);
      
      return {
        user,
        token: storedToken,
        isLoading: false,
        isAuthenticated: true
      } as AuthState;
    } else {
      yield* Effect.log("🔐 [AUTH] No stored authentication found");
      
      return {
        user: null,
        token: null,
        isLoading: false,
        isAuthenticated: false
      } as AuthState;
    }
  }).pipe(
    Effect.catchAll(error => 
      Effect.gen(function* () {
        yield* Effect.log(`Failed to check stored auth: ${error}`);
        // Clear invalid data
        yield* clearStoredAuth();
        
        return {
          user: null,
          token: null,
          isLoading: false,
          isAuthenticated: false
        } as AuthState;
      })
    )
  );

export const clearStoredAuth = () =>
  Effect.gen(function* () {
    yield* removeStorageValue(TOKEN_KEY).pipe(
      Effect.catchAll(() => Effect.void)
    );
    yield* removeStorageValue(USER_KEY).pipe(
      Effect.catchAll(() => Effect.void)
    );
  });

export const storeAuthData = (token: string, user: AuthUser) =>
  Effect.gen(function* () {
    yield* setStorageValue(TOKEN_KEY, token);
    yield* setStorageValue(USER_KEY, JSON.stringify(user));
  });

// OAuth flow for mobile (Expo)
export const startMobileOAuthFlow = (config: AuthConfig) =>
  Effect.tryPromise({
    try: async () => {
      const AuthSession = await import('expo-auth-session');
      
      const request = new AuthSession.AuthRequest({
        clientId: config.clientId,
        scopes: config.scopes,
        redirectUri: config.redirectUri,
        responseType: AuthSession.ResponseType.Code,
        state: Math.random().toString(36).substring(2, 15),
        extraParams: {
          provider: 'github'
        },
      });
      
      const result = await request.promptAsync({
        authorizationEndpoint: `${config.authUrl}/authorize`,
      });
      
      if (result.type === 'success') {
        return { code: result.params.code, state: result.params.state };
      } else if (result.type === 'cancel') {
        throw new Error('User cancelled OAuth flow');
      } else {
        throw new Error(`OAuth failed: ${result.type}`);
      }
    },
    catch: (error) => new AuthError({
      operation: "startMobileOAuthFlow",
      message: String(error),
      cause: error
    })
  });

// OAuth flow for desktop (Tauri)
export const startDesktopOAuthFlow = (config: AuthConfig) =>
  Effect.tryPromise({
    try: async () => {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      
      const state = Math.random().toString(36).substring(2, 15);
      const loginUrl = `${config.authUrl}/authorize?provider=github&redirect_uri=${encodeURIComponent(config.redirectUri)}&response_type=code&state=${state}`;
      
      await openUrl(loginUrl);
      
      // For Tauri, we need to listen for the callback
      return new Promise<{ code: string; state: string }>((resolve, reject) => {
        const handleAuthUpdate = (event: CustomEvent) => {
          const { code, state: returnedState } = event.detail;
          if (code && returnedState === state) {
            window.removeEventListener('auth-callback', handleAuthUpdate as EventListener);
            resolve({ code, state: returnedState });
          }
        };
        
        window.addEventListener('auth-callback', handleAuthUpdate as EventListener);
        
        // Timeout after 5 minutes
        setTimeout(() => {
          window.removeEventListener('auth-callback', handleAuthUpdate as EventListener);
          reject(new Error('OAuth flow timeout'));
        }, 5 * 60 * 1000);
      });
    },
    catch: (error) => new AuthError({
      operation: "startDesktopOAuthFlow",
      message: String(error),
      cause: error
    })
  });

// Exchange authorization code for token
export const exchangeCodeForToken = (config: AuthConfig, code: string, state: string) =>
  Effect.tryPromise({
    try: async () => {
      const response = await fetch(`${config.authUrl}/token`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          grant_type: 'authorization_code',
          code,
          redirect_uri: config.redirectUri,
          client_id: config.clientId,
          state,
        }),
      });
      
      if (!response.ok) {
        const errorText = await response.text();
        throw new AuthNetworkError({
          operation: "exchangeCodeForToken",
          status: response.status,
          message: `Token exchange failed: ${response.status} - ${errorText}`,
          retryable: response.status >= 500,
        });
      }
      
      return await response.json();
    },
    catch: (error) => {
      if (error instanceof AuthNetworkError) {
        throw error;
      }
      throw new AuthNetworkError({
        operation: "exchangeCodeForToken",
        message: String(error),
        retryable: true,
        cause: error
      });
    }
  });

// Complete login flow
export const login = (config: AuthConfig = getDefaultAuthConfig()) =>
  Effect.gen(function* () {
    yield* Effect.log("🔐 [AUTH] Starting OAuth login flow...");
    
    // Start OAuth flow based on platform
    const { code, state } = yield* (
      isReactNative()
        ? startMobileOAuthFlow(config)
        : startDesktopOAuthFlow(config)
    ).pipe(
      Effect.retry(
        Schedule.exponential(Duration.seconds(1)).pipe(
          Schedule.intersect(Schedule.recurs(2))
        )
      ),
      Effect.timeout(Duration.minutes(5))
    );
    
    yield* Effect.log("🔐 [AUTH] OAuth authorization received, exchanging for token...");
    
    // Exchange code for token with retry on retryable network errors
    const tokenResponse = yield* exchangeCodeForToken(config, code, state).pipe(
      Effect.retry(
        Schedule.exponential(Duration.seconds(2)).pipe(
          Schedule.intersect(Schedule.recurs(3)),
          Schedule.whileInput((error: AuthNetworkError) => error.retryable)
        )
      ),
      Effect.timeout(Duration.seconds(30))
    );
    
    // Store authentication data
    yield* storeAuthData(tokenResponse.access_token, tokenResponse.user);
    
    yield* Effect.log(`✅ [AUTH] Login successful: ${tokenResponse.user.githubUsername}`);
    
    return {
      user: tokenResponse.user,
      token: tokenResponse.access_token,
      isLoading: false,
      isAuthenticated: true
    } as AuthState;
  });

// Logout
export const logout = () =>
  Effect.gen(function* () {
    yield* Effect.log("🔐 [AUTH] Logging out...");
    
    // Clear storage
    yield* clearStoredAuth();
    
    yield* Effect.log("✅ [AUTH] Logout successful");
    
    return {
      user: null,
      token: null,
      isLoading: false,
      isAuthenticated: false
    } as AuthState;
  });

// Create auth state management
export const createAuthState = (initialState?: Partial<AuthState>) =>
  Ref.make<AuthState>({
    user: null,
    token: null,
    isLoading: true,
    isAuthenticated: false,
    ...initialState
  });

export const updateAuthState = (authStateRef: Ref.Ref<AuthState>, updates: Partial<AuthState>) =>
  Ref.update(authStateRef, state => ({ ...state, ...updates }));