import { khalaTheme } from "@effect-native/tokens"
import { randomUUID } from "expo-crypto"
import { StatusBar } from "expo-status-bar"
import * as Updates from "expo-updates"
import { useEffect, useMemo, useRef, useState } from "react"
import { SafeAreaProvider } from "react-native-safe-area-context"

import { recoverVerifiedNativeSession } from "./auth/native-session-recovery"
import { signInNativeSession, signOutNativeSession } from "./auth/native-session-pkce"
import type {
  MobileCodingDirectory,
  MobileCodingTarget,
} from "./coding/mobile-coding-navigation"
import {
  selectMobileConversation,
  type MobileConversationThread,
  type MobileConversationSelection,
} from "./conversation/mobile-conversation"
import type { MobileSyncPhase } from "./screens/home-core"
import { HomeScreen } from "./screens/home-screen"
import { openMobileSyncHost, type MobileNativeSyncHost } from "./sync/mobile-sync-host"
import { startOtaPolling } from "./updates/ota-polling"

type MobileCodingHomeBinding = Readonly<{
  directory: MobileCodingDirectory
  clearSelection: () => Promise<void>
  selectSession: (
    target: MobileCodingTarget,
    onUpdate: (thread: MobileConversationThread) => void,
  ) => Promise<MobileConversationThread | null>
}>

const selectAuthenticatedMobileExperience = async (
  syncHost: MobileNativeSyncHost,
  onActiveThread?: (thread: MobileConversationThread) => void,
): Promise<Readonly<{
  conversation: MobileConversationSelection
  coding?: MobileCodingHomeBinding
}>> => {
  const coding = syncHost.coding()
  const restored = await coding.restore()
  const preferredThreadRef = restored?.state === "ready"
    ? restored.session.threadRef
    : undefined
  const conversation = await selectMobileConversation({
    conversation: () => syncHost.conversation(),
    timeline: () => syncHost.timeline(),
    runtime: () => syncHost.runtime(),
    ...(preferredThreadRef === undefined ? {} : { preferredThreadRef }),
    adapter: { randomId: randomUUID },
  })
  const directory = await coding.directory()
  if (conversation.mode !== "sync") return { conversation }
  const host = conversation.host
  const bind = async (
    target: MobileCodingTarget,
    source: "directory" | "restore",
    onUpdate: (thread: MobileConversationThread) => void,
  ): Promise<MobileConversationThread | null> => {
    const initial = await host.openThread(target.threadRef)
    if (initial === null) return null
    let latest = initial
    const activation = await coding.activate({
      target,
      source,
      bindThread: async (threadRef, notify) => host.watchThread === undefined
        ? { close: async () => undefined }
        : host.watchThread(threadRef, thread => {
            latest = thread
            notify()
          }),
      onUpdate: () => {
        onUpdate(latest)
        onActiveThread?.(latest)
      },
    })
    if (activation.state !== "active") return null
    onActiveThread?.(latest)
    return latest
  }
  if (restored?.state === "ready") {
    await bind(restored.target, "restore", () => undefined)
  }
  return {
    conversation,
    coding: {
      directory,
      clearSelection: coding.clearActive,
      selectSession: (target, onUpdate) => bind(target, "directory", onUpdate),
    },
  }
}

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
  const [codingBinding, setCodingBinding] = useState<MobileCodingHomeBinding | undefined>()
  const [conversationRevision, setConversationRevision] = useState(0)
  const syncHostRef = useRef<MobileNativeSyncHost | null>(null)
  const syncPhaseRef = useRef<MobileSyncPhase>(syncPhase)
  useEffect(() => {
    syncPhaseRef.current = syncPhase
  }, [syncPhase])
  const applyActiveThread = (thread: MobileConversationThread): void => {
    setConversationSelection(current => current?.mode !== "sync"
      ? current
      : {
          ...current,
          activeThread: thread,
          threads: [
            {
              threadRef: thread.threadRef,
              title: thread.title,
              messageCount: thread.messageCount,
              lastMessageAt: thread.lastMessageAt,
              updatedAt: thread.updatedAt,
              version: thread.version,
            },
            ...current.threads.filter(value => value.threadRef !== thread.threadRef),
          ],
        })
  }
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
        const experience = await selectAuthenticatedMobileExperience(syncHostRef.current!, applyActiveThread)
        setConversationSelection(experience.conversation)
        setCodingBinding(experience.coding)
        setConversationRevision(current => current + 1)
        setSyncPhase(experience.conversation.mode === "sync" ? "live" : "session_ready")
        return
      }
      setSyncPhase(
        result.state === "cancelled" ? previousPhase : "unavailable",
      )
    },
    signOut: async () => {
      setSyncPhase("authenticating")
      // Revoke the callable Sync host immediately. The network token
      // revocation may still be in flight, but no captured composer can queue
      // a command that survives unlink and replays under a later session.
      try { syncHostRef.current?.unlinkAccount() } catch { /* remote revocation still runs */ }
      setCodingBinding(undefined)
      const result = await signOutNativeSession()
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
            setCodingBinding(undefined)
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
              const experience = await selectAuthenticatedMobileExperience(syncHost!, applyActiveThread)
              if (stopped) return
              setConversationSelection(experience.conversation)
              setCodingBinding(experience.coding)
              setSyncPhase(experience.conversation.mode === "sync" ? "live" : "session_ready")
            }
            break
          case "denied":
            syncHost?.unlinkAccount()
            setCodingBinding(undefined)
            setSyncPhase("denied")
            setConversationSelection({ mode: "local" })
            break
          case "unavailable":
            setCodingBinding(undefined)
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
        try { syncHost?.unlinkAccount() } catch { /* capability is already closed */ }
        setCodingBinding(undefined)
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
          coding={codingBinding}
        />
      )}
    </SafeAreaProvider>
  )
}
