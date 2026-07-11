import { khalaTheme } from "@effect-native/tokens"
import { StatusBar } from "expo-status-bar"
import * as Updates from "expo-updates"
import { useEffect, useMemo, useRef, useState } from "react"
import { SafeAreaProvider } from "react-native-safe-area-context"

import { recoverVerifiedNativeSession } from "./auth/native-session-recovery"
import { signInNativeSession, signOutNativeSession } from "./auth/native-session-pkce"
import type { MobileSyncPhase } from "./screens/home-core"
import { HomeScreen } from "./screens/home-screen"
import { openMobileSyncHost, type MobileNativeSyncHost } from "./sync/mobile-sync-host"
import { startOtaPolling } from "./updates/ota-polling"

/**
 * OpenAgents mobile (#8597) — greenfield app shell. The application/component/
 * intent model is Effect Native; this React tree is host machinery only: a
 * safe-area provider, a status bar, and the Home screen's Effect Native mount.
 *
 * Styling policy: typed style objects on the shared `@effect-native/tokens`
 * vocabulary (the Protoss-blue `khalaTheme`). No NativeWind, no Tailwind class
 * strings — see docs/effect-native/2026-07-08-styling-tailwind-stylex-effect-native.md.
 */
export const App = () => {
  const [syncPhase, setSyncPhase] = useState<MobileSyncPhase>("unconfigured")
  const syncHostRef = useRef<MobileNativeSyncHost | null>(null)
  const syncPhaseRef = useRef<MobileSyncPhase>(syncPhase)
  useEffect(() => {
    syncPhaseRef.current = syncPhase
  }, [syncPhase])
  const sessionActions = useMemo(() => ({
    signIn: async () => {
      const previousPhase = syncPhaseRef.current
      setSyncPhase("authenticating")
      const result = await signInNativeSession()
      if (result.state === "verified") {
        const connected = await syncHostRef.current?.connectStoredVerifiedSession()
        if (connected !== "connected") {
          setSyncPhase("unavailable")
          return
        }
      }
      setSyncPhase(
        result.state === "verified"
          ? "session_ready"
          : result.state === "cancelled"
            ? previousPhase
            : "unavailable",
      )
    },
    signOut: async () => {
      setSyncPhase("authenticating")
      const result = await signOutNativeSession()
      if (result.state === "signed_out") syncHostRef.current?.disconnectAuthenticated()
      setSyncPhase(result.state === "signed_out" ? "local_ready" : "unavailable")
    },
  }), [])

  // OTA: poll the owned OpenAgents Updates server (updates.openagents.com,
  // channel openagents-production) on the TEMPORARY aggressive 3s cadence —
  // see src/updates/ota-polling.ts. `Updates.isEnabled` is false in Expo Go /
  // dev, so the loop is a no-op there.
  useEffect(() => {
    let stopped = false
    let syncHost: MobileNativeSyncHost | undefined
    let localStoreReady = false
    try {
      syncHost = openMobileSyncHost()
      syncHostRef.current = syncHost
      localStoreReady = true
      setSyncPhase("local_ready")
    } catch {
      setSyncPhase("unavailable")
    }
    void recoverVerifiedNativeSession().then(
      async recovery => {
        if (stopped || !localStoreReady) return
        switch (recovery.state) {
          case "signed_out":
            setSyncPhase("local_ready")
            break
          case "verified":
            setSyncPhase(
              await syncHost?.connectStoredVerifiedSession() === "connected"
                ? "session_ready"
                : "unavailable",
            )
            break
          case "denied":
            setSyncPhase("denied")
            break
          case "unavailable":
            setSyncPhase("unavailable")
            break
        }
      },
      () => {
        if (!stopped) setSyncPhase("unavailable")
      },
    )
    const handle = startOtaPolling({
      isEnabled: Updates.isEnabled,
      checkForUpdateAsync: () => Updates.checkForUpdateAsync(),
      fetchUpdateAsync: () => Updates.fetchUpdateAsync(),
      reloadAsync: () => Updates.reloadAsync(),
    }, {
      beforeReload: () => syncHost?.close(),
    })
    return () => {
      stopped = true
      handle.stop()
      syncHost?.close()
      syncHostRef.current = null
    }
  }, [])

  return (
    <SafeAreaProvider style={{ backgroundColor: khalaTheme.color.background }}>
      <StatusBar style="light" />
      <HomeScreen syncPhase={syncPhase} sessionActions={sessionActions} />
    </SafeAreaProvider>
  )
}
