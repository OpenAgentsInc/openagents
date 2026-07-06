import { createContext, useContext, useEffect, useRef, useState } from "react"
import type { ReactNode } from "react"

import { openKhalaMobileSyncRuntime, type KhalaMobileSyncRuntime } from "./khala-mobile-sync-runtime"
import { registerActiveSyncRuntimeClose } from "./khala-mobile-sync-runtime-registry"

/**
 * Local-first chat cache (owner report: "every time i open a new thread
 * session in the app it loads the messages from scratch"). One durable
 * Khala Sync runtime — Expo SQLite store, optimistic overlay, and the
 * durable-cursor session — is opened ONCE per signed-in app session and held
 * here for the whole navigator tree. Thread screens read from it (via
 * `useKhalaSyncEntityCollection`) instead of each mounting its own
 * from-scratch bootstrap fetch, so revisiting a thread renders whatever is
 * already persisted on-device immediately while the session's own
 * durable-cursor loop (`packages/khala-sync-client/src/session.ts`
 * `driveScope`) catches up on only the rows that changed since the last
 * visit — never a full re-bootstrap once a durable cursor exists for that
 * scope.
 */
export type KhalaMobileSyncRuntimeState =
  | Readonly<{ status: "loading" }>
  | Readonly<{ status: "missing_token"; error: string }>
  | Readonly<{ status: "error"; error: string }>
  | Readonly<{ status: "ready"; runtime: KhalaMobileSyncRuntime }>

const KhalaMobileSyncRuntimeContext = createContext<KhalaMobileSyncRuntimeState | null>(null)

export type KhalaMobileSyncRuntimeProviderProps = Readonly<{
  children: ReactNode
  ownerUserId: string
  syncBaseUrl: string
  token: string
}>

export const KhalaMobileSyncRuntimeProvider = ({
  children,
  ownerUserId,
  syncBaseUrl,
  token
}: KhalaMobileSyncRuntimeProviderProps) => {
  const [state, setState] = useState<KhalaMobileSyncRuntimeState>({ status: "loading" })
  const runtimeRef = useRef<KhalaMobileSyncRuntime | null>(null)
  const unregisterCloseRef = useRef<(() => void) | null>(null)

  useEffect(() => {
    let cancelled = false
    setState({ status: "loading" })

    void (async () => {
      const opened = await openKhalaMobileSyncRuntime({
        ownerUserId,
        secureTokenLoader: async () => (token === "" ? null : token),
        syncBaseUrl
      })
      if (cancelled) {
        if (opened.ok) void opened.runtime.close()
        return
      }
      if (!opened.ok) {
        setState({
          error: opened.error,
          status: opened.authState === "missing" ? "missing_token" : "error"
        })
        return
      }
      runtimeRef.current = opened.runtime
      // Registered here (not React context) so `OtaUpdateGate` — mounted at
      // the app root, above this provider, so OTA checking still works on
      // the sign-in screen — can drain this runtime closed BEFORE
      // `Updates.reloadAsync()` tears down the JS context. See
      // `khala-mobile-sync-runtime-registry.ts` for why this exists (a real
      // production expo-sqlite native crash).
      unregisterCloseRef.current = registerActiveSyncRuntimeClose(opened.runtime.close)
      setState({ runtime: opened.runtime, status: "ready" })
    })()

    return () => {
      cancelled = true
      unregisterCloseRef.current?.()
      unregisterCloseRef.current = null
      const runtime = runtimeRef.current
      runtimeRef.current = null
      if (runtime !== null) void runtime.close()
    }
  }, [ownerUserId, syncBaseUrl, token])

  return (
    <KhalaMobileSyncRuntimeContext.Provider value={state}>
      {children}
    </KhalaMobileSyncRuntimeContext.Provider>
  )
}

export const useKhalaMobileSyncRuntime = (): KhalaMobileSyncRuntimeState => {
  const value = useContext(KhalaMobileSyncRuntimeContext)
  if (value === null) {
    throw new Error("useKhalaMobileSyncRuntime must be used within a KhalaMobileSyncRuntimeProvider")
  }
  return value
}

/** Convenience unwrap for screens that only need the low-level session /
 * overlay / store primitives to back `useKhalaSyncScopeEntities` — `null`
 * fields while the runtime is still opening (near-instant SQLite open, not
 * a network wait) or unavailable let the caller fall back to its own
 * "missing_token" / "error" messaging. */
export type KhalaMobileSyncPrimitives = Readonly<{
  error: string | null
  overlay: KhalaMobileSyncRuntime["overlay"] | null
  session: KhalaMobileSyncRuntime["session"] | null
  status: KhalaMobileSyncRuntimeState["status"]
  store: KhalaMobileSyncRuntime["store"] | null
}>

export const useKhalaMobileSyncPrimitives = (): KhalaMobileSyncPrimitives => {
  const state = useKhalaMobileSyncRuntime()
  if (state.status === "ready") {
    return {
      error: null,
      overlay: state.runtime.overlay,
      session: state.runtime.session,
      status: state.status,
      store: state.runtime.store
    }
  }
  return {
    error: state.status === "loading" ? null : state.error,
    overlay: null,
    session: null,
    status: state.status,
    store: null
  }
}
