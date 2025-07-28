import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import * as AuthSession from 'expo-auth-session';
import * as SecureStore from 'expo-secure-store';
import { Alert } from 'react-native';
import { useConfectOnboarding, UseConfectOnboardingReturn } from '@/shared/hooks/useConfectOnboarding';

interface User {
  id: string;
  email: string;
  name?: string;
  avatar?: string;
  githubId: string;
  githubUsername: string;
}

interface ConfectAuthContextType extends UseConfectOnboardingReturn {
  // Legacy auth methods for backward compatibility
  login: () => Promise<void>;
  logout: () => Promise<void>;
  token: string | null;
  
  // Enhanced onboarding state
  needsOnboarding: boolean;
  hasCompletedInitialSetup: boolean;
}

const ConfectAuthContext = createContext<ConfectAuthContextType | undefined>(undefined);

export const useConfectAuth = () => {
  const context = useContext(ConfectAuthContext);
  if (context === undefined) {
    throw new Error('useConfectAuth must be used within a ConfectAuthProvider');
  }
  return context;
};

interface ConfectAuthProviderProps {
  children: ReactNode;
}

// OpenAuth configuration
const OPENAUTH_URL = process.env.EXPO_PUBLIC_OPENAUTH_URL || 'https://auth.openagents.com';
const REDIRECT_URI = AuthSession.makeRedirectUri({ 
  scheme: 'openagents',
  path: 'auth/callback'
});

console.log('🔗 [CONFECT_AUTH] Generated REDIRECT_URI:', REDIRECT_URI);
console.log('🔗 [CONFECT_AUTH] OPENAUTH_URL:', OPENAUTH_URL);

export const ConfectAuthProvider: React.FC<ConfectAuthProviderProps> = ({ children }) => {
  const [token, setToken] = useState<string | null>(null);
  const [hasCompletedInitialSetup, setHasCompletedInitialSetup] = useState(false);
  
  // Initialize Confect onboarding system
  const confectOnboarding = useConfectOnboarding({
    autoStartOnboarding: true,
    requiredPermissions: ['notifications', 'storage', 'network'],
  });

  // Check for existing authentication on mount
  useEffect(() => {
    checkAuthState();
  }, []);

  // Check if user needs onboarding
  const needsOnboarding = confectOnboarding.isAuthenticated && 
                          !confectOnboarding.isOnboardingComplete &&
                          !confectOnboarding.onboardingState.isLoading;

  // Mark initial setup as complete when onboarding is done
  useEffect(() => {
    if (confectOnboarding.isOnboardingComplete && confectOnboarding.isAuthenticated) {
      setHasCompletedInitialSetup(true);
    }
  }, [confectOnboarding.isOnboardingComplete, confectOnboarding.isAuthenticated]);

  const checkAuthState = async () => {
    try {
      // Check if we have stored tokens in secure storage
      const storedToken = await SecureStore.getItemAsync('openauth_token');
      const storedUser = await SecureStore.getItemAsync('openauth_user');

      if (storedToken && storedUser) {
        setToken(storedToken);
        console.log('📱 [CONFECT_AUTH] Restored authentication from secure storage');
        
        // The confect hook will handle the rest of the auth setup
      } else {
        console.log('📱 [CONFECT_AUTH] No stored authentication found');
      }
    } catch (error) {
      console.error('📱 [CONFECT_AUTH] Failed to check auth state:', error);
      // Clear invalid stored data
      await clearStoredAuth();
    }
  };

  const clearStoredAuth = async () => {
    try {
      await SecureStore.deleteItemAsync('openauth_token');
      await SecureStore.deleteItemAsync('openauth_user');
    } catch (error) {
      console.error('📱 [CONFECT_AUTH] Failed to clear stored auth:', error);
    }
  };

  const login = async () => {
    try {
      console.log('📱 [CONFECT_AUTH] Starting OAuth flow with OpenAuth');

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

      console.log('📱 [CONFECT_AUTH] OAuth request configured:', {
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

      console.log('📱 [CONFECT_AUTH] OAuth result:', result.type);

      if (result.type === 'success') {
        console.log('📱 [CONFECT_AUTH] OAuth success, exchanging code for token');
        
        // Exchange authorization code for tokens
        const params = new URLSearchParams({
          grant_type: 'authorization_code',
          code: result.params.code,
          redirect_uri: REDIRECT_URI,
          client_id: 'Ov23lirHI1DWTzZ1zT1u',
        });

        console.log('📱 [CONFECT_AUTH] Token exchange params:', {
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
          console.error('📱 [CONFECT_AUTH] Token exchange failed:', response.status, errorText);
          throw new Error(`Token exchange failed: ${response.status}`);
        }

        const tokenData = await response.json();
        console.log('📱 [CONFECT_AUTH] Token exchange successful');
        
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
            setToken(tokenData.access_token);
            
            console.log('✅ [CONFECT_AUTH] Login successful:', userData.githubUsername);
            
            // The confect hook will automatically sync the user and start onboarding if needed
          } else {
            throw new Error('Failed to fetch user data');
          }
        } else {
          throw new Error('Invalid token response');
        }
      } else if (result.type === 'cancel') {
        console.log('📱 [CONFECT_AUTH] User cancelled login');
      } else {
        console.error('📱 [CONFECT_AUTH] Login failed:', result);
        Alert.alert('Login Failed', 'Unable to complete authentication. Please try again.');
      }
    } catch (error) {
      console.error('📱 [CONFECT_AUTH] Login error:', error);
      Alert.alert('Login Error', 'An error occurred during login. Please try again.');
    }
  };

  const logout = async () => {
    try {
      console.log('📱 [CONFECT_AUTH] Logging out user');
      
      // Use confect logout if available
      if (confectOnboarding.logout) {
        await confectOnboarding.logout();
      }
      
      // Clear secure storage
      await clearStoredAuth();
      
      // Clear state
      setToken(null);
      setHasCompletedInitialSetup(false);
      
      console.log('✅ [CONFECT_AUTH] Logout successful');
    } catch (error) {
      console.error('📱 [CONFECT_AUTH] Logout error:', error);
      Alert.alert('Logout Error', 'An error occurred during logout.');
    }
  };

  const value: ConfectAuthContextType = {
    // Spread all confect onboarding functionality
    ...confectOnboarding,
    
    // Legacy auth methods for backward compatibility
    login,
    logout,
    token,
    
    // Enhanced onboarding state
    needsOnboarding,
    hasCompletedInitialSetup,
  };

  return (
    <ConfectAuthContext.Provider value={value}>
      {children}
    </ConfectAuthContext.Provider>
  );
};