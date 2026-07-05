import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react"
import type { ReactNode } from "react"

import { KHALA_SYNC_DEMO_BASE_URL, KHALA_SYNC_DEMO_OWNER_USER_ID, KHALA_SYNC_DEMO_TOKEN } from "../config/khala-sync-demo"
import { discoverKhalaMobilePairing } from "./khala-mobile-pairing"
import type { KhalaMobilePairingProbeOutcome } from "./khala-mobile-pairing-core"
import { clearStoredCredentials, loadStoredCredentials, saveStoredCredentials } from "./khala-auth-store"
import { validateKhalaCredentials } from "./khala-auth-validate"

/**
 * - "loading": reading on-device secure storage; near-instant.
 * - "discovering": no stored credentials yet — probing the Tailnet for a
 *   signed-in desktop (MC-6) before ever showing manual sign-in UI.
 * - "signed_out": discovery finished without a usable pairing; the fallback
 *   screen (Tailnet retry, with manual entry as a secondary/advanced option)
 *   is shown.
 * - "signed_in": credentials resolved (stored, dev env, or auto-paired) and
 *   validated against Khala Sync.
 */
export type KhalaAuthStatus = "loading" | "discovering" | "signed_out" | "signed_in"

export type KhalaAuthState = Readonly<{
  status: KhalaAuthStatus
  baseUrl: string
  ownerUserId: string
  token: string
  /** Why auto-discovery didn't produce a signed-in session, for the fallback
   * screen's messaging. `null` before the first discovery attempt finishes. */
  discoveryOutcome: KhalaMobilePairingProbeOutcome | null
  signIn: (input: { ownerUserId: string; token: string }) => Promise<{ ok: true } | { ok: false; messageSafe: string }>
  signOut: () => Promise<void>
  /** Re-runs Tailnet auto-discovery on demand (e.g. a "Retry" button after
   * the user turns on Tailscale or signs in on their Mac). */
  retryDiscovery: () => Promise<void>
}>

const KhalaAuthContext = createContext<KhalaAuthState | null>(null)

/** The env-var pair only seeds a dev session when BOTH are present — this
 * keeps `expo start`/`expo run:ios` with exported env vars working exactly
 * as before, with no behavior change for local development. A real
 * distributed build (TestFlight, production) never has these baked in, so
 * it always falls through to Tailnet auto-discovery instead of a blank,
 * unrecoverable "Set EXPO_PUBLIC_..." screen. */
const devEnvCredentials =
  KHALA_SYNC_DEMO_OWNER_USER_ID !== "" && KHALA_SYNC_DEMO_TOKEN !== ""
    ? { ownerUserId: KHALA_SYNC_DEMO_OWNER_USER_ID, token: KHALA_SYNC_DEMO_TOKEN }
    : null

export const KhalaAuthProvider = ({ children }: { children: ReactNode }) => {
  const [status, setStatus] = useState<KhalaAuthStatus>("loading")
  const [ownerUserId, setOwnerUserId] = useState("")
  const [token, setToken] = useState("")
  const [discoveryOutcome, setDiscoveryOutcome] = useState<KhalaMobilePairingProbeOutcome | null>(null)
  const mountedRef = useRef(true)

  useEffect(() => () => {
    mountedRef.current = false
  }, [])

  const applyCredentials = useCallback(
    async (credentials: { ownerUserId: string; token: string }, persist: boolean) => {
      if (persist) await saveStoredCredentials(credentials)
      if (!mountedRef.current) return
      setOwnerUserId(credentials.ownerUserId)
      setToken(credentials.token)
      setStatus("signed_in")
    },
    []
  )

  /** Tailnet auto-auth handoff (MC-6, owner mandate 2026-07-04): before ever
   * showing a login screen, look for an already-signed-in desktop reachable
   * on the same Tailnet and pull its credentials. Only reached when no
   * stored/dev credentials already resolved the session. */
  const runDiscovery = useCallback(async () => {
    if (mountedRef.current) setStatus("discovering")
    const outcome = await discoverKhalaMobilePairing()
    if (outcome.state === "paired") {
      const validation = await validateKhalaCredentials({
        baseUrl: KHALA_SYNC_DEMO_BASE_URL,
        ownerUserId: outcome.credentials.ownerUserId,
        token: outcome.credentials.token
      })
      if (validation.ok) {
        await applyCredentials(outcome.credentials, true)
        return
      }
    }
    if (!mountedRef.current) return
    setDiscoveryOutcome(outcome)
    setStatus("signed_out")
  }, [applyCredentials])

  useEffect(() => {
    let cancelled = false
    const run = async () => {
      const stored = await loadStoredCredentials()
      if (cancelled) return
      const resolved = stored ?? devEnvCredentials
      if (resolved !== null) {
        await applyCredentials(resolved, false)
        return
      }
      await runDiscovery()
    }
    void run()
    return () => {
      cancelled = true
    }
    // Auto-discovery is intentionally only wired up once at mount via
    // `runDiscovery` (stable identity below); re-running belongs to the
    // explicit `retryDiscovery` action, not this effect.
    // eslint-disable-next-line react-hooks/exhaustive-deps
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
      await applyCredentials({ ownerUserId: trimmedOwnerUserId, token: trimmedToken }, true)
      return { ok: true as const }
    },
    [applyCredentials]
  )

  const signOut = useCallback(async () => {
    await clearStoredCredentials()
    setOwnerUserId("")
    setToken("")
    setDiscoveryOutcome(null)
    setStatus("signed_out")
  }, [])

  const value = useMemo<KhalaAuthState>(
    () => ({
      baseUrl: KHALA_SYNC_DEMO_BASE_URL,
      discoveryOutcome,
      ownerUserId,
      retryDiscovery: runDiscovery,
      signIn,
      signOut,
      status,
      token
    }),
    [discoveryOutcome, ownerUserId, runDiscovery, signIn, signOut, status, token]
  )

  return <KhalaAuthContext.Provider value={value}>{children}</KhalaAuthContext.Provider>
}

export const useKhalaAuth = (): KhalaAuthState => {
  const value = useContext(KhalaAuthContext)
  if (value === null) throw new Error("useKhalaAuth must be used within a KhalaAuthProvider")
  return value
}
