import { useCallback } from "react"
import { useAuth } from "@/context/AuthContext"

/**
 * Hook for ConvexProviderWithAuth that supplies JWT from AuthContext (WorkOS magic-link token).
 */
export function useConvexAuthFromContext(): {
  isLoading: boolean
  isAuthenticated: boolean
  fetchAccessToken: (args: { forceRefreshToken: boolean }) => Promise<string | null>
} {
  const { authToken } = useAuth()
  const fetchAccessToken = useCallback(
    async (_args: { forceRefreshToken: boolean }) => authToken ?? null,
    [authToken],
  )
  return {
    isLoading: false,
    isAuthenticated: !!authToken,
    fetchAccessToken,
  }
}
