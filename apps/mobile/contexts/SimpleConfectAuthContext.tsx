import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import * as AuthSession from 'expo-auth-session';
import * as SecureStore from 'expo-secure-store';
import { Alert } from 'react-native';
import { useSimpleConfectAuth } from '@/shared/hooks/useSimpleConfectAuth';

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
        scopes: ['user:email', 'read:user'],
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
        scopes: ['user:email', 'read:user'],
        responseType: 'code',
        provider: 'github'
      });

      // Prompt for authentication
      const result = await request.promptAsync({
        authorizationEndpoint: `${OPENAUTH_URL}/authorize`,
      });

      console.log('📱 [SIMPLE_CONFECT_AUTH] OAuth result:', result.type);

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
        
        if (tokenData.access_token) {
          // Fetch user data from OpenAuth server
          const userResponse = await fetch(`${OPENAUTH_URL}/user`, {
            headers: {
              'Authorization': `Bearer ${tokenData.access_token}`,
            },
          });
          
          if (userResponse.ok) {
            const userData = await userResponse.json();
            
            // Store tokens securely
            await SecureStore.setItemAsync('openauth_token', tokenData.access_token);
            await SecureStore.setItemAsync('openauth_user', JSON.stringify(userData));
            
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