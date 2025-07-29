import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import * as AuthSession from 'expo-auth-session';
import * as SecureStore from 'expo-secure-store';
import { Alert } from 'react-native';
// Onboarding functionality is not implemented in mobile app

interface User {
  id: string;
  email: string;
  name?: string;
  avatar?: string;
  githubId: string;
  githubUsername: string;
}

interface ConfectAuthContextType {
  // Core auth state
  user: User | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  error: string | null;
  
  // Auth methods
  login: () => Promise<void>;
  logout: () => Promise<void>;
  token: string | null;
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
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Check for existing authentication on mount
  useEffect(() => {
    checkAuthState();
  }, []);

  const checkAuthState = async () => {
    try {
      setIsLoading(true);
      
      // Check if we have stored tokens in secure storage
      const storedToken = await SecureStore.getItemAsync('openauth_token');
      const storedUser = await SecureStore.getItemAsync('openauth_user');

      if (storedToken && storedUser) {
        const userData = JSON.parse(storedUser);
        setToken(storedToken);
        setUser(userData);
        console.log('📱 [CONFECT_AUTH] Restored authentication from secure storage');
      } else {
        console.log('📱 [CONFECT_AUTH] No stored authentication found');
      }
    } catch (error) {
      console.error('📱 [CONFECT_AUTH] Failed to check auth state:', error);
      setError('Failed to restore authentication');
      // Clear invalid stored data
      await clearStoredAuth();
    } finally {
      setIsLoading(false);
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
            setUser(userData);
            setError(null);
            
            console.log('✅ [CONFECT_AUTH] Login successful:', userData.githubUsername);
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
      
      // Clear secure storage
      await clearStoredAuth();
      
      // Clear state
      setToken(null);
      setUser(null);
      setError(null);
      
      console.log('✅ [CONFECT_AUTH] Logout successful');
    } catch (error) {
      console.error('📱 [CONFECT_AUTH] Logout error:', error);
      Alert.alert('Logout Error', 'An error occurred during logout.');
    }
  };

  const value: ConfectAuthContextType = {
    // Core auth state
    user,
    isAuthenticated: !!user && !!token,
    isLoading,
    error,
    
    // Auth methods
    login,
    logout,
    token,
  };

  return (
    <ConfectAuthContext.Provider value={value}>
      {children}
    </ConfectAuthContext.Provider>
  );
};