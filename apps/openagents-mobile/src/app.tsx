import { khalaTheme } from "@effect-native/tokens"
import { randomUUID } from "expo-crypto"
import { StatusBar } from "expo-status-bar"
import * as Updates from "expo-updates"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { Linking } from "react-native"
import { SafeAreaProvider } from "react-native-safe-area-context"
import { Effect } from "effect"
import type {
  ConfirmedPortableSessionSnapshot,
  ConfirmedRuntimeAttentionSnapshot,
} from "@openagentsinc/khala-sync-client"

declare const require: (id: string) => unknown

import { recoverVerifiedNativeSession } from "./auth/native-session-recovery"
import { signInNativeSession, signOutNativeSession } from "./auth/native-session-pkce"
import type {
  MobileCodingDirectory,
  MobileCodingSelection,
  MobileCodingTarget,
} from "./coding/mobile-coding-navigation"
import {
  openMobileCodingComposer,
  type MobileCodingAttachmentUpdateResult,
  type MobileCodingComposerSession,
} from "./coding/mobile-coding-composer"
import {
  openExpoMobileCodingAttachmentDelivery,
  openExpoMobileCodingAttachmentPicker,
} from "./coding/expo-mobile-coding-attachment-picker"
import { prepareMobileCodingAttachmentDelivery } from "./coding/mobile-coding-attachment-delivery"
import type { MobileExecutionTargetOption } from "./coding/mobile-execution-targets"
import type { MobileComposerPathSearchPort } from "./coding/mobile-composer-path-context"
import type { MobileRepositoryFilesPort } from "./coding/mobile-repository-files"
import {
  buildMobilePortableSessionCommand,
  projectMobilePortableSessionControl,
  type MobilePortableControlAction,
  type MobilePortableUnavailableReason,
} from "./coding/mobile-portable-session-controls"
import {
  selectMobileConversation,
  type MobileConversationThread,
  type MobileConversationSelection,
} from "./conversation/mobile-conversation"
import {
  openMobileExperienceReconciler,
  type MobileExperienceReconciler,
} from "./conversation/mobile-experience-reconciler"
import type { MobileSyncPhase } from "./screens/home-core"
import type { FleetRunClientProjection } from "@openagentsinc/khala-sync"
import { HomeScreen } from "./screens/home-screen"
import { openMobileSyncHost, type MobileNativeSyncHost } from "./sync/mobile-sync-host"
import { startOtaPolling } from "./updates/ota-polling"
import {
  openNativeCodingTargetDelivery,
  type NativeCodingTargetDelivery,
} from "./coding/native-coding-target-delivery"
import {
  MobileAttentionTargetSchemaVersion,
  type MobileAttentionTarget,
} from "./attention/mobile-attention-target"
import {
  openNativeAttentionTargetDelivery,
  type NativeAttentionTargetDelivery,
} from "./attention/native-attention-target-delivery"
import {
  isFullAutoRunProjectionActive,
  type FullAutoRunProjectionResult,
} from "./full-auto/full-auto-run-projection"

type MobileCodingHomeBinding = Readonly<{
  directory: MobileCodingDirectory
  portableSnapshot: ConfirmedPortableSessionSnapshot | null
  attentionSnapshot: ConfirmedRuntimeAttentionSnapshot | null
  watchPortable: (listener: (snapshot: ConfirmedPortableSessionSnapshot) => void) => () => void
  watchAttention: (listener: (snapshot: ConfirmedRuntimeAttentionSnapshot) => void) => () => void
  requestPortableAction: (input: Readonly<{
    sessionRef: string
    action: MobilePortableControlAction
    destinationTargetRef?: string
  }>) => Promise<Readonly<
    | { state: "queued"; snapshot: ConfirmedPortableSessionSnapshot }
    | { state: "rejected"; reason: MobilePortableUnavailableReason; snapshot: ConfirmedPortableSessionSnapshot | null }
  >>
  activeComposer: () => MobileCodingComposerSession | null
  executionTargets: ReadonlyArray<MobileExecutionTargetOption>
  fleetRuns?: FleetRunClientProjection
  searchComposerPaths?: MobileComposerPathSearchPort["search"]
  repositoryFiles?: MobileRepositoryFilesPort
  clearSelection: () => Promise<void>
  selectSession: (
    target: MobileCodingTarget,
    onUpdate: (thread: MobileConversationThread) => void,
  ) => Promise<Readonly<{
    thread: MobileConversationThread
    composer: MobileCodingComposerSession | null
  }> | null>
  activateSession: (
    target: MobileCodingTarget,
    source: MobileCodingSelection["source"],
    onUpdate: (thread: MobileConversationThread) => void,
  ) => Promise<Readonly<{
    thread: MobileConversationThread
    composer: MobileCodingComposerSession | null
  }> | null>
  updateComposerText: (
    session: MobileCodingComposerSession,
    text: string,
  ) => Promise<MobileCodingComposerSession | null>
  selectComposerTarget: (
    session: MobileCodingComposerSession,
    target: MobileExecutionTargetOption,
  ) => Promise<MobileCodingComposerSession | null>
  pickComposerAttachments: (
    session: MobileCodingComposerSession,
  ) => Promise<MobileCodingAttachmentUpdateResult>
  removeComposerAttachment: (
    session: MobileCodingComposerSession,
    attachmentId: string,
  ) => Promise<MobileCodingComposerSession | null>
  retryComposerAttachment: (
    session: MobileCodingComposerSession,
    attachmentId: string,
  ) => Promise<MobileCodingComposerSession | null>
  prepareComposerSubmission: (
    session: MobileCodingComposerSession,
    message: string,
  ) => ReturnType<typeof prepareMobileCodingAttachmentDelivery>
  clearComposer: (
    session: MobileCodingComposerSession,
  ) => Promise<MobileCodingComposerSession | null>
}>

const selectAuthenticatedMobileExperience = async (
  syncHost: MobileNativeSyncHost,
  onActiveThread?: (thread: MobileConversationThread) => void,
): Promise<Readonly<{
  conversation: MobileConversationSelection
  coding?: MobileCodingHomeBinding
  fullAutoRun: FullAutoRunProjectionResult
}>> => {
  const coding = syncHost.coding()
  const draftStore = syncHost.drafts()
  const composer = draftStore === null
    ? null
    : openMobileCodingComposer({ drafts: draftStore, randomId: randomUUID })
  const attachmentPicker = openExpoMobileCodingAttachmentPicker()
  const attachmentDelivery = openExpoMobileCodingAttachmentDelivery()
  const restored = await coding.restore()
  const preferredThreadRef = restored?.state === "ready"
    ? restored.session.threadRef
    : undefined
  // Fetched before thread selection so an active, fresh Full Auto run's
  // thread can take priority over the arbitrary `threads[0]` fallback
  // (openagents #8982) — see `selectActiveConversationThreadRef`. An
  // explicit restored coding session (`preferredThreadRef`) still wins.
  const fullAutoRunResult = await syncHost.fullAutoRun()
  // `threadRef` is nullable: a Full Auto run may exist before Desktop binds
  // it to a khala-sync thread. `undefined` here means "nothing to
  // prioritize", not "no active run" — the live state header can still
  // appear later once the run's thread does match the selected one.
  const activeFullAutoThreadRef = fullAutoRunResult.state === "active" &&
      fullAutoRunResult.projection.threadRef !== null &&
      isFullAutoRunProjectionActive(fullAutoRunResult.projection)
    ? fullAutoRunResult.projection.threadRef
    : undefined
  const conversation = await selectMobileConversation({
    conversation: () => syncHost.conversation(),
    timeline: () => syncHost.timeline(),
    agentGraph: () => syncHost.agentGraph(),
    runtime: () => syncHost.runtime(),
    interactions: () => syncHost.interactions(),
    ...(preferredThreadRef === undefined ? {} : { preferredThreadRef }),
    ...(activeFullAutoThreadRef === undefined ? {} : { activeFullAutoThreadRef }),
    adapter: { randomId: randomUUID },
  })
  const directory = await coding.directory()
  if (conversation.mode !== "sync") return { conversation, fullAutoRun: fullAutoRunResult }
  const executionTargetCatalog = await syncHost.executionTargets()
  const fleetRunResult = await syncHost.fleetRuns()
  const repositoryEnvironment = await syncHost.repositoryEnvironment()
  const portable = syncHost.portable()
  const portableSnapshot = portable === null
    ? null
    : await Effect.runPromise(portable.snapshot())
  const attention = syncHost.attention()
  const attentionSnapshot = attention === null
    ? null
    : await Effect.runPromise(attention.snapshot())
  const executionTargets = executionTargetCatalog?.options ?? []
  const host = conversation.host
  let activeComposer: MobileCodingComposerSession | null = null
  const bind = async (
    target: MobileCodingTarget,
    source: MobileCodingSelection["source"],
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
          executionTargets,
          effectiveExecutionTargetId: executionTargetCatalog?.effectiveTargetId,
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
    fullAutoRun: fullAutoRunResult,
    coding: {
      directory,
      portableSnapshot,
      attentionSnapshot,
      watchPortable: listener => syncHost.watchPortable(listener),
      watchAttention: listener => syncHost.watchAttention(listener),
      requestPortableAction: async input => {
        if (portable === null) {
          return { state: "rejected", reason: "authority_unavailable", snapshot: null }
        }
        const latest = await Effect.runPromise(portable.snapshot())
        const control = projectMobilePortableSessionControl(latest, input.sessionRef)
        const built = buildMobilePortableSessionCommand({
          control,
          action: input.action,
          invocationRef: `tap.${randomUUID()}`,
          issuedAt: new Date().toISOString(),
          ...(input.destinationTargetRef === undefined
            ? {}
            : { destinationTargetRef: input.destinationTargetRef }),
        })
        if (built.state === "rejected") {
          return { state: "rejected", reason: built.reason, snapshot: latest }
        }
        await Effect.runPromise(portable.request(built.command))
        return {
          state: "queued",
          snapshot: await Effect.runPromise(portable.snapshot()),
        }
      },
      activeComposer: () => activeComposer,
      executionTargets,
      ...(repositoryEnvironment === null
        ? {}
        : {
            repositoryFiles: repositoryEnvironment,
            searchComposerPaths: repositoryEnvironment.search,
          }),
      ...(fleetRunResult.state === "available"
        ? { fleetRuns: fleetRunResult.projection }
        : {}),
      clearSelection: async () => {
        activeComposer = null
        await coding.clearActive()
      },
      selectSession: (target, onUpdate) => bind(target, "directory", onUpdate),
      activateSession: (target, source, onUpdate) => bind(target, source, onUpdate),
      updateComposerText: async (session, text) => {
        if (composer === null) return null
        const updated = await composer.updateText(session, text)
        if (updated !== null) activeComposer = updated
        return updated
      },
      selectComposerTarget: async (session, target) => {
        if (composer === null ||
          !executionTargets.some(option =>
            option.targetId === target.targetId && option.readiness === "ready")) return null
        const updated = await composer.selectTarget(session, target)
        if (updated !== null) activeComposer = updated
        return updated
      },
      removeComposerAttachment: async (session, attachmentId) => {
        if (composer === null) return null
        const updated = await composer.removeAttachment(session, attachmentId)
        if (updated !== null) activeComposer = updated
        return updated
      },
      retryComposerAttachment: async (session, attachmentId) => {
        if (composer === null) return null
        const attachment = session.draft.doc.attachments.find(candidate =>
          candidate.id === attachmentId && candidate.status === "error")
        if (attachment?.digest === undefined) return null
        try {
          const bytes = await attachmentDelivery.read(attachment.digest)
          const digest = (await attachmentDelivery.sha256(bytes)).toLowerCase()
          if (bytes.byteLength !== attachment.sizeBytes ||
            digest !== attachment.digest.toLowerCase()) return null
          const updated = await composer.retryAttachment(session, attachmentId, {
            digest,
            sizeBytes: bytes.byteLength,
          })
          if (updated !== null) activeComposer = updated
          return updated
        } catch {
          return null
        }
      },
      prepareComposerSubmission: (session, message) =>
        prepareMobileCodingAttachmentDelivery({
          message,
          attachments: session.draft.doc.attachments,
          port: attachmentDelivery,
        }),
      clearComposer: async session => {
        if (composer === null) return null
        const updated = await composer.clear(session)
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
  const [pendingAttentionTarget, setPendingAttentionTarget] = useState<MobileAttentionTarget | null>(null)
  const [conversationRevision, setConversationRevision] = useState(0)
  const [fullAutoRun, setFullAutoRun] = useState<FullAutoRunProjectionResult | null>(null)
  const syncHostRef = useRef<MobileNativeSyncHost | null>(null)
  const codingBindingRef = useRef<MobileCodingHomeBinding | undefined>(undefined)
  const targetDeliveryRef = useRef<NativeCodingTargetDelivery | null>(null)
  const attentionDeliveryRef = useRef<NativeAttentionTargetDelivery | null>(null)
  const pendingAttentionTargetRef = useRef<MobileAttentionTarget | null>(null)
  const portableSubscriptionRef = useRef<(() => void) | null>(null)
  const attentionSubscriptionRef = useRef<(() => void) | null>(null)
  const syncPhaseRef = useRef<MobileSyncPhase>(syncPhase)
  const consumeAttentionTarget = useCallback((): void => {
    pendingAttentionTargetRef.current = null
    setPendingAttentionTarget(null)
    void attentionDeliveryRef.current?.flush()
  }, [])
  useEffect(() => {
    syncPhaseRef.current = syncPhase
  }, [syncPhase])
  // The pre-live conversation read (in signIn/boot hydration) is necessarily
  // the local fallback; this ref lets the phase poller re-evaluate the choice
  // once the scope reaches live without re-running a single-shot read.
  const conversationSelectionRef = useRef<MobileConversationSelection | null>(null)
  useEffect(() => {
    conversationSelectionRef.current = conversationSelection
  }, [conversationSelection])
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
              status: thread.status,
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
    portableSubscriptionRef.current?.()
    portableSubscriptionRef.current = null
    attentionSubscriptionRef.current?.()
    attentionSubscriptionRef.current = null
    codingBindingRef.current = binding
    setCodingBinding(binding)
    if (binding !== undefined) {
      portableSubscriptionRef.current = binding.watchPortable(snapshot => {
        const current = codingBindingRef.current
        if (current === undefined) return
        const updated = { ...current, portableSnapshot: snapshot }
        codingBindingRef.current = updated
        setCodingBinding(updated)
      })
      attentionSubscriptionRef.current = binding.watchAttention(snapshot => {
        const current = codingBindingRef.current
        if (current === undefined) return
        const updated = { ...current, attentionSnapshot: snapshot }
        codingBindingRef.current = updated
        setCodingBinding(updated)
        void attentionDeliveryRef.current?.flush()
      })
    }
    if (binding !== undefined) void targetDeliveryRef.current?.flush()
    if (binding !== undefined) void attentionDeliveryRef.current?.flush()
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
        setFullAutoRun(experience.fullAutoRun)
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
      pendingAttentionTargetRef.current = null
      setPendingAttentionTarget(null)
      setFullAutoRun(null)
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
    let fullAutoRunPollTimer: ReturnType<typeof setInterval> | undefined
    let fullAutoRunPollInFlight = false
    let linkSubscription: ReturnType<typeof Linking.addEventListener> | undefined
    let notificationSubscription: Readonly<{ remove: () => void }> | undefined
    let targetDelivery: NativeCodingTargetDelivery | undefined
    let attentionDelivery: NativeAttentionTargetDelivery | undefined
    let experienceReconciler: MobileExperienceReconciler | undefined
    let localStoreReady = false
    try {
      syncHost = openMobileSyncHost()
      syncHostRef.current = syncHost
      experienceReconciler = openMobileExperienceReconciler({
        currentMode: () => conversationSelectionRef.current?.mode ?? "local",
        needsRefresh: () =>
          codingBindingRef.current?.directory.authority !== "confirmed",
        // A non-null callable conversation host means a verified session is
        // connected and its personal scope is live — the exact precondition for
        // upgrading the local fallback to the confirmed sync surface.
        isAuthenticatedLive: () => syncHost?.conversation() != null,
        selectExperience: () =>
          selectAuthenticatedMobileExperience(syncHost!, applyActiveThread),
        onUpgrade: experience => {
          conversationSelectionRef.current = experience.conversation
          setConversationSelection(experience.conversation)
          setFullAutoRun(experience.fullAutoRun)
          publishCodingBinding(experience.coding)
          setConversationRevision(current => current + 1)
          setSyncPhase("live")
        },
      })
      targetDelivery = openNativeCodingTargetDelivery({
        resolve: candidate => syncHost!.coding().accept(candidate),
        activate: async (target, source) => {
          const binding = codingBindingRef.current
          if (binding === undefined) return false
          const selected = await binding.activateSession(target, source, applyActiveThread)
          return selected !== null
        },
      })
      targetDeliveryRef.current = targetDelivery
      attentionDelivery = openNativeAttentionTargetDelivery({
        snapshot: () => codingBindingRef.current?.attentionSnapshot ?? null,
        deliver: target => {
          if (pendingAttentionTargetRef.current !== null) return false
          pendingAttentionTargetRef.current = target
          setPendingAttentionTarget(target)
          return true
        },
      })
      attentionDeliveryRef.current = attentionDelivery
      const enqueueCoding = (candidate: Parameters<NativeCodingTargetDelivery["enqueue"]>[0]): void => {
        targetDelivery?.enqueue(candidate)
        void targetDelivery?.flush()
      }
      const enqueueAttention = (candidate: Parameters<NativeAttentionTargetDelivery["enqueue"]>[0]): void => {
        attentionDelivery?.enqueue(candidate)
        void attentionDelivery?.flush()
      }
      const isAttentionUrl = (url: string): boolean => {
        try {
          const parsed = new URL(url)
          return parsed.protocol === "openagents:" && parsed.hostname === "attention"
        } catch {
          return false
        }
      }
      const isAttentionPayload = (payload: unknown): boolean =>
        typeof payload === "object" && payload !== null && !Array.isArray(payload) &&
        (payload as { schema?: unknown }).schema === MobileAttentionTargetSchemaVersion
      linkSubscription = Linking.addEventListener("url", event => {
        if (isAttentionUrl(event.url)) enqueueAttention({ source: "deep_link", url: event.url })
        else enqueueCoding({ source: "deep_link", url: event.url })
      })
      void Linking.getInitialURL().then(url => {
        if (url === null) return
        if (isAttentionUrl(url)) enqueueAttention({ source: "deep_link", url })
        else enqueueCoding({ source: "deep_link", url })
      })
      void Promise.resolve().then(async () => {
        const Notifications = require("expo-notifications") as typeof import("expo-notifications")
        if (stopped) return
        notificationSubscription = Notifications.addNotificationResponseReceivedListener(response => {
          const payload = response.notification.request.content.data
          if (isAttentionPayload(payload)) enqueueAttention({ source: "notification", payload })
          else enqueueCoding({ source: "notification", payload })
        })
        const initial = await Notifications.getLastNotificationResponseAsync()
        if (initial !== null) {
          const payload = initial.notification.request.content.data
          if (isAttentionPayload(payload)) enqueueAttention({ source: "notification", payload })
          else enqueueCoding({ source: "notification", payload })
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
              setFullAutoRun(experience.fullAutoRun)
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
      if (phase === "live") void attentionDelivery?.flush()
      // Re-evaluate local vs sync once the scope reaches live: the pre-live
      // read fell back to local, and this upgrades it to the confirmed sync
      // surface (authority "sync", "Continue conversation") exactly once.
      experienceReconciler?.observePhase(phase === "closed" ? undefined : phase)
    }, 250)
    // Live Full Auto run projection poll (openagents #8982): a lightweight
    // interval, independent of the 250ms sync-status timer, so the state
    // header's lifecycle state updates without an app restart. Only polls
    // while a live synced conversation exists; a no-op elsewhere, so it adds
    // no behavior when there is no active run or no signed-in session.
    fullAutoRunPollTimer = setInterval(() => {
      if (stopped || fullAutoRunPollInFlight) return
      if (syncHost?.conversation() == null) return
      fullAutoRunPollInFlight = true
      void syncHost.fullAutoRun()
        .then(result => {
          if (!stopped) setFullAutoRun(result)
        })
        .finally(() => {
          fullAutoRunPollInFlight = false
        })
    }, 5_000)
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
      if (fullAutoRunPollTimer !== undefined) clearInterval(fullAutoRunPollTimer)
      experienceReconciler?.close()
      handle.stop()
      linkSubscription?.remove()
      notificationSubscription?.remove()
      targetDelivery?.close()
      targetDeliveryRef.current = null
      attentionDelivery?.close()
      attentionDeliveryRef.current = null
      portableSubscriptionRef.current?.()
      portableSubscriptionRef.current = null
      attentionSubscriptionRef.current?.()
      attentionSubscriptionRef.current = null
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
          pendingAttentionTarget={pendingAttentionTarget}
          onAttentionTargetConsumed={consumeAttentionTarget}
          fullAutoRun={fullAutoRun}
        />
      )}
    </SafeAreaProvider>
  )
}
