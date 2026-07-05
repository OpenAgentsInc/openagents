import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react"
import type { ReactNode } from "react"

import { KHALA_SYNC_DEMO_BASE_URL, KHALA_SYNC_DEMO_OWNER_USER_ID, KHALA_SYNC_DEMO_TOKEN } from "../config/khala-sync-demo"
import { clearStoredCredentials, loadStoredCredentials, saveStoredCredentials } from "./khala-auth-store"
import { validateKhalaCredentials } from "./khala-auth-validate"

export type KhalaAuthStatus = "loading" | "signed_out" | "signed_in"

export type KhalaAuthState = Readonly<{
  status: KhalaAuthStatus
  baseUrl: string
  ownerUserId: string
  token: string
  signIn: (input: { ownerUserId: string; token: string }) => Promise<{ ok: true } | { ok: false; messageSafe: string }>
  signOut: () => Promise<void>
}>

const KhalaAuthContext = createContext<KhalaAuthState | null>(null)

/** The env-var pair only seeds a dev session when BOTH are present — this
 * keeps `expo start`/`expo run:ios` with exported env vars working exactly
 * as before, with no behavior change for local development. A real
 * distributed build (TestFlight, production) never has these baked in, so
 * it always falls through to the real sign-in screen instead of a blank,
 * unrecoverable "Set EXPO_PUBLIC_..." screen. */
const devEnvCredentials =
  KHALA_SYNC_DEMO_OWNER_USER_ID !== "" && KHALA_SYNC_DEMO_TOKEN !== ""
    ? { ownerUserId: KHALA_SYNC_DEMO_OWNER_USER_ID, token: KHALA_SYNC_DEMO_TOKEN }
    : null

export const KhalaAuthProvider = ({ children }: { children: ReactNode }) => {
  const [status, setStatus] = useState<KhalaAuthStatus>("loading")
  const [ownerUserId, setOwnerUserId] = useState("")
  const [token, setToken] = useState("")

  useEffect(() => {
    let cancelled = false
    const run = async () => {
      const stored = await loadStoredCredentials()
      if (cancelled) return
      const resolved = stored ?? devEnvCredentials
      if (resolved === null) {
        setStatus("signed_out")
        return
      }
      setOwnerUserId(resolved.ownerUserId)
      setToken(resolved.token)
      setStatus("signed_in")
    }
    void run()
    return () => {
      cancelled = true
    }
  }, [])

  const signIn = useCallback(
    async (input: { ownerUserId: string; token: string }) => {
      const trimmedOwnerUserId = input.ownerUserId.trim()
      const trimmedToken = input.token.trim()
      if (trimmedOwnerUserId === "" || trimmedToken === "") {
        return { messageSafe: "Owner user id and token are both required.", ok: false as const }
      }
      const validation = await validateKhalaCredentials({
        baseUrl: KHALA_SYNC_DEMO_BASE_URL,
        ownerUserId: trimmedOwnerUserId,
        token: trimmedToken
      })
      if (!validation.ok) return validation
      await saveStoredCredentials({ ownerUserId: trimmedOwnerUserId, token: trimmedToken })
      setOwnerUserId(trimmedOwnerUserId)
      setToken(trimmedToken)
      setStatus("signed_in")
      return { ok: true as const }
    },
    []
  )

  const signOut = useCallback(async () => {
    await clearStoredCredentials()
    setOwnerUserId("")
    setToken("")
    setStatus("signed_out")
  }, [])

  const value = useMemo<KhalaAuthState>(
    () => ({ baseUrl: KHALA_SYNC_DEMO_BASE_URL, ownerUserId, signIn, signOut, status, token }),
    [ownerUserId, signIn, signOut, status, token]
  )

  return <KhalaAuthContext.Provider value={value}>{children}</KhalaAuthContext.Provider>
}

export const useKhalaAuth = (): KhalaAuthState => {
  const value = useContext(KhalaAuthContext)
  if (value === null) throw new Error("useKhalaAuth must be used within a KhalaAuthProvider")
  return value
}
