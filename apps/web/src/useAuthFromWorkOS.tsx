import { useAccessToken, useAuth } from '@workos/authkit-tanstack-react-start/client';
import { useCallback, useMemo } from 'react';

/**
 * Auth hook for ConvexProviderWithAuth. Use inside AuthKitProvider.
 */
export function useAuthFromWorkOS() {
  const { loading, user } = useAuth();
  const { accessToken, getAccessToken } = useAccessToken();

  const fetchAccessToken = useCallback(
    async ({ forceRefreshToken }: { forceRefreshToken: boolean }) => {
      if (!accessToken || forceRefreshToken) {
        return (await getAccessToken()) ?? null;
      }

      return accessToken;
    },
    [accessToken, getAccessToken],
  );

  return useMemo(
    () => ({
      // Don't block UI on auth check: show content immediately (unauthenticated until WorkOS resolves).
      isLoading: false,
      isAuthenticated: !!user,
      fetchAccessToken,
    }),
    [user, fetchAccessToken],
  );
}
