import { useCallback } from "react"

import { useAuth } from "@/context/AuthContext"

/**
 * Hook for ConvexProviderWithAuth. Supplies WorkOS token from AuthContext so Convex
 * can authenticate the user (same JWT issuer as apps/web).
 */
export function useConvexAuth(): {
  isLoading: boolean
  isAuthenticated: boolean
  fetchAccessToken: (args: { forceRefreshToken: boolean }) => Promise<string | null>
} {
  const { isAuthenticated, authToken } = useAuth()

  const fetchAccessToken = useCallback(
    async (_args: { forceRefreshToken: boolean }) => {
      return authToken ?? null
    },
    [authToken],
  )

  return {
    isLoading: false,
    isAuthenticated,
    fetchAccessToken,
  }
}
