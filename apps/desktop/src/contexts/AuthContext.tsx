import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';

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
      // Check if we have stored tokens
      const storedToken = localStorage.getItem('openauth_token');
      const storedUser = localStorage.getItem('openauth_user');

      if (storedToken && storedUser) {
        setToken(storedToken);
        setUser(JSON.parse(storedUser));
      }
    } catch (error) {
      console.error('Failed to check auth state:', error);
      // Clear invalid stored data
      localStorage.removeItem('openauth_token');
      localStorage.removeItem('openauth_user');
    } finally {
      setIsLoading(false);
    }
  };

  const login = async () => {
    try {
      setIsLoading(true);
      
      // Import Tauri API for invoking backend commands and events
      const { invoke } = await import('@tauri-apps/api/core');
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      const { listen } = await import('@tauri-apps/api/event');
      
      console.log('🔐 Starting OAuth server...');
      
      // Start OAuth server and get the port
      const port = await invoke('start_oauth_server') as number;
      console.log('🌐 OAuth server started on port:', port);
      
      // Build OAuth URL with the actual port
      const authUrl = import.meta.env.VITE_OPENAUTH_URL || 'https://auth.openagents.com';
      const redirectUri = `http://localhost:${port}/callback`;
      const loginUrl = `${authUrl}/authorize?provider=github&client_id=desktop&redirect_uri=${encodeURIComponent(redirectUri)}&response_type=code`;
      
      console.log('🌐 Opening OAuth URL:', loginUrl);
      
      // Set up event listener for OAuth success
      const oauthPromise = new Promise<{ code: string; state?: string }>((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('OAuth flow timed out'));
        }, 300000); // 5 minutes timeout
        
        // Listen for OAuth success event
        listen('oauth_success', (event) => {
          clearTimeout(timeout);
          console.log('✅ OAuth success event received:', event.payload);
          resolve(event.payload as { code: string; state?: string });
        });
        
        // Listen for OAuth error event
        listen('oauth_error', (event) => {
          clearTimeout(timeout);
          console.error('❌ OAuth error event received:', event.payload);
          reject(new Error(`OAuth failed: ${event.payload}`));
        });
      });
      
      // Open OAuth URL in browser
      await openUrl(loginUrl);
      
      // Wait for OAuth callback
      const oauthResult = await oauthPromise;
      
      if (oauthResult?.code) {
        console.log('✅ OAuth code received, exchanging for tokens...');
        
        // Exchange code for access token and user info in one call
        const tokenResponse = await invoke('exchange_oauth_code', {
          code: oauthResult.code,
          clientId: 'desktop',
          redirectUri: redirectUri
        }) as { access_token: string; token_type: string; user: User };
        
        if (tokenResponse?.access_token && tokenResponse?.user) {
          console.log('✅ Access token and user info received!');
          
          // Store tokens and user info
          localStorage.setItem('openauth_token', tokenResponse.access_token);
          localStorage.setItem('openauth_user', JSON.stringify(tokenResponse.user));
          
          setToken(tokenResponse.access_token);
          setUser(tokenResponse.user);
          
          console.log('🎉 Authentication successful!');
        } else {
          throw new Error('No access token or user info received');
        }
      } else {
        throw new Error('No authorization code received');
      }
      
    } catch (error) {
      console.error('❌ Login failed:', error);
      throw error;
    } finally {
      setIsLoading(false);
    }
  };

  const logout = async () => {
    try {
      setIsLoading(true);
      
      // Clear local storage
      localStorage.removeItem('openauth_token');
      localStorage.removeItem('openauth_user');
      
      // Clear state
      setToken(null);
      setUser(null);
      
    } catch (error) {
      console.error('Logout failed:', error);
    } finally {
      setIsLoading(false);
    }
  };

  // Listen for auth updates from Tauri backend
  useEffect(() => {
    const handleAuthUpdate = (event: CustomEvent) => {
      const { token: newToken, user: newUser } = event.detail;
      
      if (newToken && newUser) {
        setToken(newToken);
        setUser(newUser);
        localStorage.setItem('openauth_token', newToken);
        localStorage.setItem('openauth_user', JSON.stringify(newUser));
      }
    };

    window.addEventListener('auth-update', handleAuthUpdate as EventListener);
    
    return () => {
      window.removeEventListener('auth-update', handleAuthUpdate as EventListener);
    };
  }, []);

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