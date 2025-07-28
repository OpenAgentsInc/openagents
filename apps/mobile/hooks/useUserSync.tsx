import { useEffect, useState } from 'react';
import { useMutation } from 'convex/react';
import { api } from '../convex/_generated/api';
import { useConfectAuth } from '../contexts/SimpleConfectAuthContext';

/**
 * Hook that automatically creates/syncs the user record in Convex
 * when the user is authenticated
 * 
 * @returns {object} Object with isSynced status to prevent race conditions
 */
export const useUserSync = () => {
  const { isAuthenticated, user } = useConfectAuth();
  const getOrCreateUser = useMutation(api.users.getOrCreateUser);
  const [isSynced, setIsSynced] = useState(false);

  useEffect(() => {
    const syncUser = async () => {
      if (!isAuthenticated || !user) {
        setIsSynced(false);
        return;
      }

      setIsSynced(false); // Reset sync status
      
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
  }, [isAuthenticated, user, getOrCreateUser]);

  return { isSynced };
};