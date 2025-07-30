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
      
      // Redirect to OpenAuth server with desktop client configuration
      const authUrl = import.meta.env.VITE_OPENAUTH_URL || 'https://auth.openagents.com';
      const redirectUri = 'http://localhost:8080/auth/callback'; // Desktop uses localhost callback
      
      const loginUrl = `${authUrl}/authorize?provider=github&client_id=desktop&redirect_uri=${encodeURIComponent(redirectUri)}&response_type=code`;
      
      // Open external URL using Tauri opener plugin
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl(loginUrl);
      
      // TODO: Set up localhost server to handle callback
      // For now, the callback will need to be handled by a temporary server
      
    } catch (error) {
      console.error('Login failed:', error);
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