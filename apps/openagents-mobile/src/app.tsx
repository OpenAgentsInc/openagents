import { khalaTheme } from "@effect-native/tokens"
import { randomUUID } from "expo-crypto"
import { StatusBar } from "expo-status-bar"
import * as Updates from "expo-updates"
import { useEffect, useMemo, useRef, useState } from "react"
import { SafeAreaProvider } from "react-native-safe-area-context"

import { recoverVerifiedNativeSession } from "./auth/native-session-recovery"
import { signInNativeSession, signOutNativeSession } from "./auth/native-session-pkce"
import {
  selectMobileConversation,
  type MobileConversationSelection,
} from "./conversation/mobile-conversation"
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
  const [conversationSelection, setConversationSelection] = useState<MobileConversationSelection | null>(null)
  const [conversationRevision, setConversationRevision] = useState(0)
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
          setConversationSelection({ mode: "local" })
          setConversationRevision(current => current + 1)
          setSyncPhase("unavailable")
          return
        }
        const selection = await selectMobileConversation({
          conversation: () => syncHostRef.current?.conversation() ?? null,
          adapter: { randomId: randomUUID },
        })
        setConversationSelection(selection)
        setConversationRevision(current => current + 1)
        setSyncPhase(selection.mode === "sync" ? "live" : "session_ready")
        return
      }
      setSyncPhase(
        result.state === "cancelled" ? previousPhase : "unavailable",
      )
    },
    signOut: async () => {
      setSyncPhase("authenticating")
      const result = await signOutNativeSession()
      if (result.state === "signed_out") syncHostRef.current?.unlinkAccount()
      if (result.state === "signed_out") {
        setConversationSelection({ mode: "local" })
        setConversationRevision(current => current + 1)
      }
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
    let syncStatusTimer: ReturnType<typeof setInterval> | undefined
    let localStoreReady = false
    try {
      syncHost = openMobileSyncHost()
      syncHostRef.current = syncHost
      localStoreReady = true
      setSyncPhase("local_ready")
    } catch {
      setSyncPhase("unavailable")
      setConversationSelection({ mode: "local" })
    }
    void recoverVerifiedNativeSession().then(
      async recovery => {
        if (stopped || !localStoreReady) return
        switch (recovery.state) {
          case "signed_out":
            setSyncPhase("local_ready")
            setConversationSelection({ mode: "local" })
            break
          case "verified":
            if (await syncHost?.connectStoredVerifiedSession() !== "connected") {
              setSyncPhase("unavailable")
              setConversationSelection({ mode: "local" })
              break
            }
            {
              const selection = await selectMobileConversation({
                conversation: () => syncHost?.conversation() ?? null,
                adapter: { randomId: randomUUID },
              })
              if (stopped) return
              setConversationSelection(selection)
              setSyncPhase(selection.mode === "sync" ? "live" : "session_ready")
            }
            break
          case "denied":
            syncHost?.unlinkAccount()
            setSyncPhase("denied")
            setConversationSelection({ mode: "local" })
            break
          case "unavailable":
            setSyncPhase("unavailable")
            setConversationSelection({ mode: "local" })
            break
        }
      },
      () => {
        if (!stopped) {
          setSyncPhase("unavailable")
          setConversationSelection({ mode: "local" })
        }
      },
    )
    syncStatusTimer = setInterval(() => {
      const phase = syncHost?.status().syncPhase
      if (phase === "denied" && syncPhaseRef.current !== "denied") {
        syncPhaseRef.current = "denied"
        setConversationSelection({ mode: "local" })
        setConversationRevision(current => current + 1)
      }
      if (
        phase === "bootstrapping" || phase === "catching_up" || phase === "live" ||
        phase === "must_refetch" || phase === "denied"
      ) setSyncPhase(phase)
    }, 250)
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
      if (syncStatusTimer !== undefined) clearInterval(syncStatusTimer)
      handle.stop()
      syncHost?.close()
      syncHostRef.current = null
    }
  }, [])

  return (
    <SafeAreaProvider style={{ backgroundColor: khalaTheme.color.background }}>
      <StatusBar style="light" />
      {conversationSelection === null ? null : (
        <HomeScreen
          key={`conversation-${conversationRevision}-${conversationSelection.mode}`}
          syncPhase={syncPhase}
          sessionActions={sessionActions}
          conversation={conversationSelection.mode === "sync" ? conversationSelection : undefined}
        />
      )}
    </SafeAreaProvider>
  )
}
