import React, { useEffect, ReactNode, useMemo } from 'react';
import { ConvexProvider, ConvexReactClient } from 'convex/react';
import { useAuth } from '@/contexts/AuthContext';

interface ConvexProviderWithAuthProps {
  children: ReactNode;
}

export const ConvexProviderWithAuth: React.FC<ConvexProviderWithAuthProps> = ({ children }) => {
  const { token, isAuthenticated } = useAuth();

  // Create Convex client once and reuse it
  const convex = useMemo(() => {
    const convexUrl = import.meta.env.VITE_CONVEX_URL;
    if (!convexUrl) {
      console.error('❌ [CONVEX] VITE_CONVEX_URL not configured');
      return null;
    }

    console.log('🔧 [CONVEX] Creating ConvexReactClient for desktop');
    return new ConvexReactClient(convexUrl, {
      unsavedChangesWarning: false,
    });
  }, []);

  // Update authentication when token changes
  useEffect(() => {
    if (!convex) return;

    if (isAuthenticated && token) {
      console.log('🔑 [CONVEX] Setting authentication token for desktop');
      console.log('🔍 [CONVEX] Token preview:', token.substring(0, 50) + '...');
      
      // Decode JWT to check its contents (for debugging)
      try {
        const payload = JSON.parse(atob(token.split('.')[1]));
        console.log('🔍 [CONVEX] JWT payload:', {
          iss: payload.iss,
          aud: payload.aud,
          sub: payload.sub,
          exp: payload.exp,
          iat: payload.iat
        });
        
        // Check if JWT has required fields
        if (!payload.iat) {
          console.warn('⚠️ [CONVEX] JWT missing iat field - this may cause auth issues');
        }
      } catch (e) {
        console.error('❌ [CONVEX] Failed to decode JWT:', e);
      }
      
      // Clear any existing auth first, then set new auth
      convex.clearAuth();
      
      // Add small delay to ensure clearAuth completes
      setTimeout(() => {
        console.log('🔑 [CONVEX] Setting new authentication for desktop');
        convex.setAuth(async () => {
          console.log('🔑 [CONVEX] Auth function called, returning token');
          return token;
        });
      }, 100);
    } else {
      console.log('🔓 [CONVEX] Clearing authentication for desktop');
      convex.clearAuth();
    }
  }, [convex, token, isAuthenticated]);

  // Show loading state while Convex client is being initialized
  if (!convex) {
    return null;
  }

  return (
    <ConvexProvider client={convex}>
      {children}
    </ConvexProvider>
  );
};