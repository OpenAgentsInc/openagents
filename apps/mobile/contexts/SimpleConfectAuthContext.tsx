import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import * as AuthSession from 'expo-auth-session';
import * as SecureStore from 'expo-secure-store';
import { Alert } from 'react-native';
import { useSimpleConfectAuth } from '../../../packages/shared/src/hooks/useSimpleConfectAuth';

interface User {
  id: string;
  email: string;
  name?: string;
  avatar?: string;
  githubId: string;
  githubUsername: string;
}

interface SimpleConfectAuthContextType {
  // Auth state
  user: User | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  error: string | null;
  token: string | null;
  
  // Auth methods
  login: () => Promise<void>;
  logout: () => Promise<void>;
  forceLogout: () => Promise<void>; // Debug method
  
  // Onboarding state (simplified)
  needsOnboarding: boolean;
  hasCompletedInitialSetup: boolean;
  markOnboardingComplete: () => void;
}

const SimpleConfectAuthContext = createContext<SimpleConfectAuthContextType | undefined>(undefined);

export const useConfectAuth = () => {
  const context = useContext(SimpleConfectAuthContext);
  if (context === undefined) {
    throw new Error('useConfectAuth must be used within a SimpleConfectAuthProvider');
  }
  return context;
};

interface SimpleConfectAuthProviderProps {
  children: ReactNode;
}

// OpenAuth configuration
const OPENAUTH_URL = process.env.EXPO_PUBLIC_OPENAUTH_URL || 'https://auth.openagents.com';
const REDIRECT_URI = AuthSession.makeRedirectUri({ 
  scheme: 'openagents',
  path: 'auth/callback'
});

console.log('🔗 [SIMPLE_CONFECT_AUTH] Generated REDIRECT_URI:', REDIRECT_URI);
console.log('🔗 [SIMPLE_CONFECT_AUTH] OPENAUTH_URL:', OPENAUTH_URL);

export const SimpleConfectAuthProvider: React.FC<SimpleConfectAuthProviderProps> = ({ children }) => {
  // Use the simplified Confect auth hook
  const authHook = useSimpleConfectAuth({
    convexUrl: process.env.EXPO_PUBLIC_CONVEX_URL,
  });

  const [hasCompletedInitialSetup, setHasCompletedInitialSetup] = useState(false);
  const [customToken, setCustomToken] = useState<string | null>(null);
  const [customUser, setCustomUser] = useState<User | null>(null);
  const [customError, setCustomError] = useState<string | null>(null);

  // Override with actual OAuth implementation for now
  const isAuthenticated = !!(authHook.user ?? customUser) && !!(authHook.token ?? customToken);
  
  // Check if user needs onboarding (simplified logic)
  const needsOnboarding = isAuthenticated && !hasCompletedInitialSetup;

  // Check for existing authentication on mount
  useEffect(() => {
    checkAuthState();
  }, []);

  const checkAuthState = async () => {
    try {
      // Check if we have stored tokens in secure storage
      const storedToken = await SecureStore.getItemAsync('openauth_token');
      const storedUser = await SecureStore.getItemAsync('openauth_user');
      const storedOnboardingComplete = await SecureStore.getItemAsync('onboarding_complete');

      if (storedToken && storedUser) {
        setCustomToken(storedToken);
        setCustomUser(JSON.parse(storedUser));
        setHasCompletedInitialSetup(storedOnboardingComplete === 'true');
        console.log('📱 [SIMPLE_CONFECT_AUTH] Restored authentication from secure storage');
      } else {
        console.log('📱 [SIMPLE_CONFECT_AUTH] No stored authentication found');
      }
    } catch (error) {
      console.error('📱 [SIMPLE_CONFECT_AUTH] Failed to check auth state:', error);
      // Clear invalid stored data
      await clearStoredAuth();
      setCustomError('Failed to restore authentication');
    }
  };

  const clearStoredAuth = async () => {
    try {
      await SecureStore.deleteItemAsync('openauth_token');
      await SecureStore.deleteItemAsync('openauth_user');
      await SecureStore.deleteItemAsync('onboarding_complete');
    } catch (error) {
      console.error('📱 [SIMPLE_CONFECT_AUTH] Failed to clear stored auth:', error);
    }
  };

  const login = async () => {
    try {
      setCustomError(null);
      console.log('📱 [SIMPLE_CONFECT_AUTH] Starting OAuth flow with OpenAuth');

      // Create OAuth request
      const request = new AuthSession.AuthRequest({
        clientId: 'Ov23lirHI1DWTzZ1zT1u', // GitHub OAuth App Client ID
        scopes: ['user:email', 'read:user', 'repo'],
        redirectUri: REDIRECT_URI,
        responseType: AuthSession.ResponseType.Code,
        state: Math.random().toString(36).substring(2, 15),
        usePKCE: false, // Disable PKCE for now
        extraParams: {
          provider: 'github'
        },
      });

      console.log('📱 [SIMPLE_CONFECT_AUTH] OAuth request configured:', {
        clientId: 'Ov23lirHI1DWTzZ1zT1u',
        redirectUri: REDIRECT_URI,
        authUrl: OPENAUTH_URL,
        scopes: ['user:email', 'read:user', 'repo'],
        responseType: 'code',
        provider: 'github'
      });

      // Prompt for authentication
      const result = await request.promptAsync({
        authorizationEndpoint: `${OPENAUTH_URL}/authorize`,
      });

      console.log('📱 [SIMPLE_CONFECT_AUTH] OAuth result:', result.type);
      console.log('📱 [SIMPLE_CONFECT_AUTH] Full OAuth result:', result);

      if (result.type === 'success') {
        console.log('📱 [SIMPLE_CONFECT_AUTH] OAuth success, exchanging code for token');
        
        // Exchange authorization code for tokens
        const params = new URLSearchParams({
          grant_type: 'authorization_code',
          code: result.params.code,
          redirect_uri: REDIRECT_URI,
          client_id: 'Ov23lirHI1DWTzZ1zT1u',
        });

        console.log('📱 [SIMPLE_CONFECT_AUTH] Token exchange params:', {
          grant_type: 'authorization_code',
          code: result.params.code,
          redirect_uri: REDIRECT_URI,
          client_id: 'Ov23lirHI1DWTzZ1zT1u',
        });

        const response = await fetch(`${OPENAUTH_URL}/token`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/x-www-form-urlencoded',
          },
          body: params.toString(),
        });

        if (!response.ok) {
          const errorText = await response.text();
          console.error('📱 [SIMPLE_CONFECT_AUTH] Token exchange failed:', response.status, errorText);
          throw new Error(`Token exchange failed: ${response.status}`);
        }

        const tokenData = await response.json();
        console.log('📱 [SIMPLE_CONFECT_AUTH] Token exchange successful');
        console.log('🔍 [SIMPLE_CONFECT_AUTH] Raw token response:', tokenData);
        console.log('🔍 [SIMPLE_CONFECT_AUTH] Token data structure:', {
          hasAccessToken: !!tokenData.access_token,
          hasRefreshToken: !!tokenData.refresh_token,
          hasGithubToken: !!tokenData.github_access_token,
          hasProviderTokens: !!tokenData.provider_tokens,
          tokenKeys: Object.keys(tokenData),
          tokenDataType: typeof tokenData,
          isArray: Array.isArray(tokenData),
        });

        // Check if GitHub token is directly in the token response
        let initialGithubToken = null;
        if (tokenData.github_access_token) {
          initialGithubToken = tokenData.github_access_token;
          console.log('✅ [SIMPLE_CONFECT_AUTH] GitHub token found in token exchange response');
        } else if (tokenData.provider_tokens?.github) {
          initialGithubToken = tokenData.provider_tokens.github;
          console.log('✅ [SIMPLE_CONFECT_AUTH] GitHub token found in provider_tokens');
        }
        
        if (tokenData.access_token) {
          // Fetch user data from OpenAuth server
          const userResponse = await fetch(`${OPENAUTH_URL}/user`, {
            headers: {
              'Authorization': `Bearer ${tokenData.access_token}`,
            },
          });

          // Also try to get provider tokens (GitHub access token)
          const tokensResponse = await fetch(`${OPENAUTH_URL}/tokens`, {
            headers: {
              'Authorization': `Bearer ${tokenData.access_token}`,
            },
          });
          
          if (userResponse.ok) {
            const userData = await userResponse.json();
            console.log('👤 [SIMPLE_CONFECT_AUTH] Raw user response:', userData);
            
            // Try to get GitHub token from tokens endpoint or initial token response
            let githubAccessToken = initialGithubToken; // Start with token from initial response
            
            if (tokensResponse.ok) {
              const tokensData = await tokensResponse.json();
              console.log('🔑 [SIMPLE_CONFECT_AUTH] Raw tokens response:', tokensData);
              console.log('🔑 [SIMPLE_CONFECT_AUTH] Tokens data structure:', {
                tokensKeys: Object.keys(tokensData),
                hasGithubToken: !!tokensData.github,
                hasProviders: !!tokensData.providers,
                tokensDataType: typeof tokensData,
                isArray: Array.isArray(tokensData),
              });
              
              // Extract GitHub token from tokens response (prefer this over initial)
              if (tokensData.github?.access_token) {
                githubAccessToken = tokensData.github.access_token;
                console.log('✅ [SIMPLE_CONFECT_AUTH] GitHub access token found in tokens response');
              } else if (tokensData.providers?.github?.access_token) {
                githubAccessToken = tokensData.providers.github.access_token;
                console.log('✅ [SIMPLE_CONFECT_AUTH] GitHub access token found in providers.github');
              }
            } else {
              console.warn('⚠️ [SIMPLE_CONFECT_AUTH] Failed to fetch tokens:', tokensResponse.status);
              console.log('💡 [SIMPLE_CONFECT_AUTH] Using GitHub token from initial response if available');
            }
            
            // Final fallback: check if GitHub token is in user data
            if (!githubAccessToken && userData.githubAccessToken) {
              githubAccessToken = userData.githubAccessToken;
              console.log('✅ [SIMPLE_CONFECT_AUTH] GitHub access token found in user data');
            }
            
            console.log('📊 [SIMPLE_CONFECT_AUTH] User data received:', {
              githubUsername: userData.githubUsername,
              hasGithubToken: !!githubAccessToken,
              tokenKeys: Object.keys(userData).filter(k => k.includes('token')),
              allKeys: Object.keys(userData),
              fullUserData: userData, // Log everything to debug
            });
            
            // Store tokens securely
            await SecureStore.setItemAsync('openauth_token', tokenData.access_token);
            await SecureStore.setItemAsync('openauth_user', JSON.stringify(userData));
            
            // Store GitHub access token separately if available
            if (githubAccessToken) {
              await SecureStore.setItemAsync('github_access_token', githubAccessToken);
              console.log('💾 [SIMPLE_CONFECT_AUTH] GitHub access token stored in secure storage');
            } else {
              console.warn('⚠️ [SIMPLE_CONFECT_AUTH] No GitHub access token available to store');
            }
            
            // Update state
            setCustomToken(tokenData.access_token);
            setCustomUser(userData);
            
            console.log('✅ [SIMPLE_CONFECT_AUTH] Login successful:', userData.githubUsername);
          } else {
            throw new Error('Failed to fetch user data');
          }
        } else {
          throw new Error('Invalid token response');
        }
      } else if (result.type === 'cancel') {
        console.log('📱 [SIMPLE_CONFECT_AUTH] User cancelled login');
      } else {
        console.error('📱 [SIMPLE_CONFECT_AUTH] Login failed:', result);
        Alert.alert('Login Failed', 'Unable to complete authentication. Please try again.');
      }
    } catch (error) {
      console.error('📱 [SIMPLE_CONFECT_AUTH] Login error:', error);
      setCustomError(error instanceof Error ? error.message : 'Login failed');
      Alert.alert('Login Error', 'An error occurred during login. Please try again.');
    }
  };

  const logout = async () => {
    try {
      console.log('📱 [SIMPLE_CONFECT_AUTH] Logging out user');
      
      // Clear secure storage
      await clearStoredAuth();
      
      // Clear custom state
      setCustomToken(null);
      setCustomUser(null);
      setHasCompletedInitialSetup(false);
      setCustomError(null);
      
      // Call authHook logout
      await authHook.logout();
      
      console.log('✅ [SIMPLE_CONFECT_AUTH] Logout successful');
    } catch (error) {
      console.error('📱 [SIMPLE_CONFECT_AUTH] Logout error:', error);
      setCustomError(error instanceof Error ? error.message : 'Logout failed');
      Alert.alert('Logout Error', 'An error occurred during logout.');
    }
  };

  // Debug method to force clear all auth data
  const forceLogout = async () => {
    try {
      console.log('🔄 [SIMPLE_CONFECT_AUTH] Force logout - clearing ALL auth data');
      
      // Clear ALL stored auth data including GitHub token
      await clearStoredAuth();
      await SecureStore.deleteItemAsync('github_access_token');
      
      // Clear custom state
      setCustomToken(null);
      setCustomUser(null);
      setHasCompletedInitialSetup(false);
      setCustomError(null);
      
      // Call authHook logout
      await authHook.logout();
      
      console.log('✅ [SIMPLE_CONFECT_AUTH] Force logout completed - please log in again');
    } catch (error) {
      console.error('📱 [SIMPLE_CONFECT_AUTH] Force logout error:', error);
    }
  };

  const markOnboardingComplete = async () => {
    try {
      await SecureStore.setItemAsync('onboarding_complete', 'true');
      setHasCompletedInitialSetup(true);
      console.log('✅ [SIMPLE_CONFECT_AUTH] Onboarding marked as complete');
    } catch (error) {
      console.error('📱 [SIMPLE_CONFECT_AUTH] Failed to mark onboarding complete:', error);
    }
  };

  const value: SimpleConfectAuthContextType = {
    user: authHook.user ?? customUser,
    isAuthenticated,
    isLoading: authHook.isLoading,
    error: authHook.error ?? customError,
    token: authHook.token ?? customToken,
    login,
    logout,
    forceLogout, // Debug method
    needsOnboarding,
    hasCompletedInitialSetup,
    markOnboardingComplete,
  };

  return (
    <SimpleConfectAuthContext.Provider value={value}>
      {children}
    </SimpleConfectAuthContext.Provider>
  );
};