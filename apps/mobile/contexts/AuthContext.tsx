import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import * as AuthSession from 'expo-auth-session';
import * as SecureStore from 'expo-secure-store';
import { Alert } from 'react-native';

interface User {
  id: string;
  email: string;
  name?: string;
  avatar?: string;
  githubId: string;
  githubUsername: string;
}

interface AuthContextType {
  user: User | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  login: () => Promise<void>;
  logout: () => Promise<void>;
  token: string | null;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export const useAuth = () => {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
};

interface AuthProviderProps {
  children: ReactNode;
}

// OpenAuth configuration
const OPENAUTH_URL = process.env.EXPO_PUBLIC_OPENAUTH_URL || 'https://auth.openagents.com';
const REDIRECT_URI = AuthSession.makeRedirectUri({ 
  scheme: 'openagents',
  path: 'auth/callback'
});

export const AuthProvider: React.FC<AuthProviderProps> = ({ children }) => {
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [token, setToken] = useState<string | null>(null);

  const isAuthenticated = !!user && !!token;

  // Check for existing authentication on mount
  useEffect(() => {
    checkAuthState();
  }, []);

  const checkAuthState = async () => {
    try {
      // Check if we have stored tokens in secure storage
      const storedToken = await SecureStore.getItemAsync('openauth_token');
      const storedUser = await SecureStore.getItemAsync('openauth_user');

      if (storedToken && storedUser) {
        setToken(storedToken);
        setUser(JSON.parse(storedUser));
        console.log('📱 [AUTH] Restored authentication from secure storage');
      } else {
        console.log('📱 [AUTH] No stored authentication found');
      }
    } catch (error) {
      console.error('📱 [AUTH] Failed to check auth state:', error);
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
      console.error('📱 [AUTH] Failed to clear stored auth:', error);
    }
  };

  const login = async () => {
    try {
      setIsLoading(true);
      console.log('📱 [AUTH] Starting OAuth flow with OpenAuth');

      // Create OAuth request
      const request = new AuthSession.AuthRequest({
        clientId: 'openagents-mobile', // This should match your OpenAuth configuration
        scopes: ['openid', 'profile', 'email'],
        redirectUri: REDIRECT_URI,
        responseType: AuthSession.ResponseType.Code,
        state: Math.random().toString(36).substring(2, 15),
        extraParams: {
          provider: 'github'
        },
      });

      console.log('📱 [AUTH] OAuth request configured:', {
        redirectUri: REDIRECT_URI,
        authUrl: OPENAUTH_URL
      });

      // Prompt for authentication
      const result = await request.promptAsync({
        authorizationEndpoint: `${OPENAUTH_URL}/authorize`,
      });

      console.log('📱 [AUTH] OAuth result:', result.type);

      if (result.type === 'success') {
        console.log('📱 [AUTH] OAuth success, exchanging code for token');
        
        // Exchange authorization code for tokens
        const tokenResponse = await exchangeCodeForToken(result.params.code, result.params.state);
        
        if (tokenResponse.access_token && tokenResponse.user) {
          // Store tokens securely
          await SecureStore.setItemAsync('openauth_token', tokenResponse.access_token);
          await SecureStore.setItemAsync('openauth_user', JSON.stringify(tokenResponse.user));
          
          // Update state
          setToken(tokenResponse.access_token);
          setUser(tokenResponse.user);
          
          console.log('✅ [AUTH] Login successful:', tokenResponse.user.githubUsername);
        } else {
          throw new Error('Invalid token response');
        }
      } else if (result.type === 'cancel') {
        console.log('📱 [AUTH] User cancelled login');
      } else {
        console.error('📱 [AUTH] Login failed:', result);
        Alert.alert('Login Failed', 'Unable to complete authentication. Please try again.');
      }
    } catch (error) {
      console.error('📱 [AUTH] Login error:', error);
      Alert.alert('Login Error', 'An error occurred during login. Please try again.');
    } finally {
      setIsLoading(false);
    }
  };

  const exchangeCodeForToken = async (code: string, state: string) => {
    console.log('📱 [AUTH] Exchanging authorization code for tokens');
    
    const response = await fetch(`${OPENAUTH_URL}/token`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        grant_type: 'authorization_code',
        code,
        redirect_uri: REDIRECT_URI,
        client_id: 'openagents-mobile',
        state,
      }),
    });

    if (!response.ok) {
      const errorText = await response.text();
      console.error('📱 [AUTH] Token exchange failed:', response.status, errorText);
      throw new Error(`Token exchange failed: ${response.status}`);
    }

    const tokenData = await response.json();
    console.log('📱 [AUTH] Token exchange successful');
    
    return tokenData;
  };

  const logout = async () => {
    try {
      setIsLoading(true);
      console.log('📱 [AUTH] Logging out user');
      
      // Clear secure storage
      await clearStoredAuth();
      
      // Clear state
      setToken(null);
      setUser(null);
      
      console.log('✅ [AUTH] Logout successful');
    } catch (error) {
      console.error('📱 [AUTH] Logout error:', error);
      Alert.alert('Logout Error', 'An error occurred during logout.');
    } finally {
      setIsLoading(false);
    }
  };

  const value = {
    user,
    isAuthenticated,
    isLoading,
    login,
    logout,
    token,
  };

  return (
    <AuthContext.Provider value={value}>
      {children}
    </AuthContext.Provider>
  );
};