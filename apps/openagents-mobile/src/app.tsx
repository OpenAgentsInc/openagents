import { khalaTheme } from "@effect-native/tokens"
import { randomUUID } from "expo-crypto"
import { StatusBar } from "expo-status-bar"
import * as Updates from "expo-updates"
import { useEffect, useMemo, useRef, useState } from "react"
import { Linking } from "react-native"
import { SafeAreaProvider } from "react-native-safe-area-context"

import { recoverVerifiedNativeSession } from "./auth/native-session-recovery"
import { signInNativeSession, signOutNativeSession } from "./auth/native-session-pkce"
import type {
  MobileCodingDirectory,
  MobileCodingTarget,
} from "./coding/mobile-coding-navigation"
import {
  openMobileCodingComposer,
  type MobileCodingAttachmentUpdateResult,
  type MobileCodingComposerSession,
} from "./coding/mobile-coding-composer"
import { openExpoMobileCodingAttachmentPicker } from "./coding/expo-mobile-coding-attachment-picker"
import {
  selectMobileConversation,
  type MobileConversationThread,
  type MobileConversationSelection,
} from "./conversation/mobile-conversation"
import type { MobileSyncPhase } from "./screens/home-core"
import { HomeScreen } from "./screens/home-screen"
import { openMobileSyncHost, type MobileNativeSyncHost } from "./sync/mobile-sync-host"
import { startOtaPolling } from "./updates/ota-polling"
import {
  openNativeCodingTargetDelivery,
  type NativeCodingTargetDelivery,
} from "./coding/native-coding-target-delivery"

type MobileCodingHomeBinding = Readonly<{
  directory: MobileCodingDirectory
  activeComposer: () => MobileCodingComposerSession | null
  clearSelection: () => Promise<void>
  selectSession: (
    target: MobileCodingTarget,
    onUpdate: (thread: MobileConversationThread) => void,
  ) => Promise<Readonly<{
    thread: MobileConversationThread
    composer: MobileCodingComposerSession | null
  }> | null>
  updateComposerText: (
    session: MobileCodingComposerSession,
    text: string,
  ) => Promise<MobileCodingComposerSession | null>
  pickComposerAttachments: (
    session: MobileCodingComposerSession,
  ) => Promise<MobileCodingAttachmentUpdateResult>
}>

const selectAuthenticatedMobileExperience = async (
  syncHost: MobileNativeSyncHost,
  onActiveThread?: (thread: MobileConversationThread) => void,
): Promise<Readonly<{
  conversation: MobileConversationSelection
  coding?: MobileCodingHomeBinding
}>> => {
  const coding = syncHost.coding()
  const draftStore = syncHost.drafts()
  const composer = draftStore === null
    ? null
    : openMobileCodingComposer({ drafts: draftStore, randomId: randomUUID })
  const attachmentPicker = openExpoMobileCodingAttachmentPicker()
  const restored = await coding.restore()
  const preferredThreadRef = restored?.state === "ready"
    ? restored.session.threadRef
    : undefined
  const conversation = await selectMobileConversation({
    conversation: () => syncHost.conversation(),
    timeline: () => syncHost.timeline(),
    agentGraph: () => syncHost.agentGraph(),
    runtime: () => syncHost.runtime(),
    interactions: () => syncHost.interactions(),
    ...(preferredThreadRef === undefined ? {} : { preferredThreadRef }),
    adapter: { randomId: randomUUID },
  })
  const directory = await coding.directory()
  if (conversation.mode !== "sync") return { conversation }
  const host = conversation.host
  let activeComposer: MobileCodingComposerSession | null = null
  const bind = async (
    target: MobileCodingTarget,
    source: "directory" | "restore",
    onUpdate: (thread: MobileConversationThread) => void,
  ): Promise<Readonly<{
    thread: MobileConversationThread
    composer: MobileCodingComposerSession | null
  }> | null> => {
    const resolution = await coding.resolve(target)
    if (resolution.state !== "ready") return null
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
    const composerSession = composer === null
      ? null
      : await composer.open({
          target,
          resolution,
          runtime: latest.timeline?.run?.runtime,
        })
    activeComposer = composerSession
    onActiveThread?.(latest)
    return { thread: latest, composer: composerSession }
  }
  if (restored?.state === "ready") {
    await bind(restored.target, "restore", () => undefined)
  }
  return {
    conversation,
    coding: {
      directory,
      activeComposer: () => activeComposer,
      clearSelection: async () => {
        activeComposer = null
        await coding.clearActive()
      },
      selectSession: (target, onUpdate) => bind(target, "directory", onUpdate),
      updateComposerText: async (session, text) => {
        if (composer === null) return null
        const updated = await composer.updateText(session, text)
        if (updated !== null) activeComposer = updated
        return updated
      },
      pickComposerAttachments: async session => {
        if (composer === null) {
          return { status: "failed", error: "Private draft storage is unavailable." }
        }
        const picked = await attachmentPicker.pick()
        if (picked.status !== "selected") return picked
        const updated = await composer.addAttachments(session, picked.files)
        if (updated === null) {
          return {
            status: "failed",
            error: "Those files or images could not be added to this draft.",
          }
        }
        activeComposer = updated
        return {
          status: "updated",
          session: updated,
          addedCount: picked.files.length,
        }
      },
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
  const codingBindingRef = useRef<MobileCodingHomeBinding | undefined>(undefined)
  const targetDeliveryRef = useRef<NativeCodingTargetDelivery | null>(null)
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
  const publishCodingBinding = (binding: MobileCodingHomeBinding | undefined): void => {
    codingBindingRef.current = binding
    setCodingBinding(binding)
    if (binding !== undefined) void targetDeliveryRef.current?.flush()
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
        publishCodingBinding(experience.coding)
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
      publishCodingBinding(undefined)
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
    let linkSubscription: ReturnType<typeof Linking.addEventListener> | undefined
    let notificationSubscription: Readonly<{ remove: () => void }> | undefined
    let targetDelivery: NativeCodingTargetDelivery | undefined
    let localStoreReady = false
    try {
      syncHost = openMobileSyncHost()
      syncHostRef.current = syncHost
      targetDelivery = openNativeCodingTargetDelivery({
        resolve: candidate => syncHost!.coding().accept(candidate),
        activate: async target => {
          const binding = codingBindingRef.current
          if (binding === undefined) return false
          const selected = await binding.selectSession(target, applyActiveThread)
          return selected !== null
        },
      })
      targetDeliveryRef.current = targetDelivery
      const enqueue = (candidate: Parameters<NativeCodingTargetDelivery["enqueue"]>[0]): void => {
        targetDelivery?.enqueue(candidate)
        void targetDelivery?.flush()
      }
      linkSubscription = Linking.addEventListener("url", event => {
        enqueue({ source: "deep_link", url: event.url })
      })
      void Linking.getInitialURL().then(url => {
        if (url !== null) enqueue({ source: "deep_link", url })
      })
      void import("expo-notifications").then(async Notifications => {
        if (stopped) return
        notificationSubscription = Notifications.addNotificationResponseReceivedListener(response => {
          enqueue({ source: "notification", payload: response.notification.request.content.data })
        })
        const initial = await Notifications.getLastNotificationResponseAsync()
        if (initial !== null) {
          enqueue({ source: "notification", payload: initial.notification.request.content.data })
        }
      })
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
            publishCodingBinding(undefined)
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
              publishCodingBinding(experience.coding)
              setSyncPhase(experience.conversation.mode === "sync" ? "live" : "session_ready")
            }
            break
          case "denied":
            syncHost?.unlinkAccount()
            publishCodingBinding(undefined)
            setSyncPhase("denied")
            setConversationSelection({ mode: "local" })
            break
          case "unavailable":
            publishCodingBinding(undefined)
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
        publishCodingBinding(undefined)
        syncPhaseRef.current = "denied"
        setConversationSelection({ mode: "local" })
        setConversationRevision(current => current + 1)
      }
      if (
        phase === "bootstrapping" || phase === "catching_up" || phase === "live" ||
        phase === "must_refetch" || phase === "denied"
      ) setSyncPhase(phase)
      if (phase === "live") void targetDelivery?.flush()
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
      linkSubscription?.remove()
      notificationSubscription?.remove()
      targetDelivery?.close()
      targetDeliveryRef.current = null
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
