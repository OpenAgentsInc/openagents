import { useCallback, useMemo } from 'react';
import { useAccessToken, useAuth } from '@workos/authkit-tanstack-react-start/client';

/** Convex auth adapter for WorkOS AuthKit. Use inside AuthKitProvider. */
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
      isLoading: loading,
      isAuthenticated: !!user,
      fetchAccessToken,
    }),
    [loading, user, fetchAccessToken],
  );
}
