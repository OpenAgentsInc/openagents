import { useEffect, useState } from 'react';
import { useMutation } from 'convex/react';
import * as SecureStore from 'expo-secure-store';
import { api } from '../convex/_generated/api';
import { useConfectAuth } from '../contexts/SimpleConfectAuthContext';

/**
 * Hook that automatically creates/syncs the user record in Convex
 * when the user is authenticated
 * 
 * @returns {object} Object with isSynced status to prevent race conditions
 */
export const useUserSync = () => {
  const { isAuthenticated, user, token } = useConfectAuth();
  const getOrCreateUser = useMutation(api.confect.users.getOrCreateUser);
  const [isSynced, setIsSynced] = useState(false);

  useEffect(() => {
    const syncUser = async () => {
      if (!isAuthenticated || !user || !token) {
        setIsSynced(false);
        return;
      }

      setIsSynced(false); // Reset sync status
      
      // Parse JWT to get OpenAuth subject
      let openAuthSubject: string | undefined;
      try {
        const payload = JSON.parse(atob(token.split('.')[1]));
        openAuthSubject = payload.sub;
      } catch (error) {
        console.warn('⚠️ [USER SYNC] Failed to parse JWT token:', error);
      }
      
      // Get GitHub access token from secure storage
      let githubAccessToken: string | undefined;
      try {
        githubAccessToken = await SecureStore.getItemAsync('github_access_token');
        console.log('🔑 [USER SYNC] GitHub token found:', !!githubAccessToken);
        if (githubAccessToken) {
          console.log('🔍 [USER SYNC] GitHub token preview:', githubAccessToken.substring(0, 20) + '...');
        }
        
        // Also try to get it from the user data in case it's stored there
        const storedUser = await SecureStore.getItemAsync('openauth_user');
        if (storedUser) {
          const userData = JSON.parse(storedUser);
          console.log('📊 [USER SYNC] Stored user data keys:', Object.keys(userData));
          if (userData.githubAccessToken) {
            console.log('🔍 [USER SYNC] Found GitHub token in user data');
            githubAccessToken = userData.githubAccessToken;
          }
        }
      } catch (error) {
        console.warn('⚠️ [USER SYNC] Failed to get GitHub token:', error);
      }
      
      // Add delay to ensure Convex auth is fully established
      await new Promise(resolve => setTimeout(resolve, 1000));

      try {
        console.log('👤 [USER SYNC] Creating/updating user in Convex:', user.githubUsername);
        
        await getOrCreateUser({
          email: user.email,
          name: user.name,
          avatar: user.avatar,
          githubId: user.githubId,
          githubUsername: user.githubUsername,
          openAuthSubject,
          githubAccessToken: githubAccessToken || undefined, // Convert null to undefined for schema
        });

        console.log('✅ [USER SYNC] User synced successfully');
        setIsSynced(true); // Mark as synced
      } catch (error) {
        console.error('❌ [USER SYNC] Failed to sync user:', error);
        // Retry once after another delay
        setTimeout(async () => {
          try {
            console.log('🔄 [USER SYNC] Retrying user sync...');
            await getOrCreateUser({
              email: user.email,
              name: user.name,
              avatar: user.avatar,
              githubId: user.githubId,
              githubUsername: user.githubUsername,
              openAuthSubject,
              githubAccessToken: githubAccessToken || undefined, // Convert null to undefined for schema
            });
            console.log('✅ [USER SYNC] User synced successfully on retry');
            setIsSynced(true); // Mark as synced on retry success
          } catch (retryError) {
            console.error('❌ [USER SYNC] Retry failed:', retryError);
            setIsSynced(false); // Keep as not synced if retry fails
          }
        }, 2000);
      }
    };

    syncUser();
  }, [isAuthenticated, user, token, getOrCreateUser]);

  return { isSynced };
};