import { useCallback } from "react"

import { useAuth } from "@/context/AuthContext"
import { mintConvexToken } from "@/services/runtimeCodexApi"

let cachedConvexToken: {
  authToken: string
  convexToken: string
  expiresAtMs: number
} | null = null

/**
 * Hook for ConvexProviderWithAuth that mints short-lived Convex JWTs via
 * Laravel `/api/convex/token` using the authenticated mobile bearer token.
 */
export function useConvexAuthFromContext(): {
  isLoading: boolean
  isAuthenticated: boolean
  fetchAccessToken: (args: { forceRefreshToken: boolean }) => Promise<string | null>
} {
  const { authToken } = useAuth()

  const fetchAccessToken = useCallback(
    async ({ forceRefreshToken }: { forceRefreshToken: boolean }) => {
      if (!authToken) return null

      const now = Date.now()
      if (
        !forceRefreshToken &&
        cachedConvexToken &&
        cachedConvexToken.authToken === authToken &&
        cachedConvexToken.expiresAtMs > now + 30_000
      ) {
        return cachedConvexToken.convexToken
      }

      try {
        const minted = await mintConvexToken(authToken)
        const expiresAtMs = now + Math.max(30, minted.expires_in) * 1000

        cachedConvexToken = {
          authToken,
          convexToken: minted.token,
          expiresAtMs,
        }

        return minted.token
      } catch {
        return null
      }
    },
    [authToken],
  )

  return {
    isLoading: false,
    isAuthenticated: !!authToken,
    fetchAccessToken,
  }
}
