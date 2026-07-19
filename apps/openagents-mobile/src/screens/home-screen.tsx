import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { File } from "expo-file-system"
import {
  AccessibilityInfo,
  AppState,
  KeyboardAvoidingView,
  Platform,
  View as RNView,
  useWindowDimensions,
} from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { khalaTheme } from "@effect-native/tokens"
import type { IntentReporter } from "@effect-native/core"

import type { MobileCodingDirectory, MobileCodingTarget } from "../coding/mobile-coding-navigation"
import type {
  MobileCodingAttachmentUpdateResult,
  MobileCodingComposerSession,
} from "../coding/mobile-coding-composer"
import type { MobileExecutionTargetOption } from "../coding/mobile-execution-targets"
import type { MobileComposerPathSearchPort } from "../coding/mobile-composer-path-context"
import type { MobileRepositoryFilesPort } from "../coding/mobile-repository-files"
import type { MobileRepositoryGitPort } from "../coding/mobile-repository-git"
import type { MobileRepositoryReviewPort } from "../coding/mobile-repository-review"
import type { MobileRepositoryTerminalPort } from "../coding/mobile-repository-terminal"
import type {
  MobileEnvironmentConnectionsPort,
  MobileNotificationSettingsPort,
  MobileShareIntake,
} from "../settings/mobile-settings"
import type {
  ConfirmedPortableSessionSnapshot,
  ConfirmedRuntimeAttentionSnapshot,
} from "@openagentsinc/khala-sync-client"
import type { MobileAttentionTarget } from "../attention/mobile-attention-target"
import type {
  MobilePortableControlAction,
  MobilePortableUnavailableReason,
} from "../coding/mobile-portable-session-controls"
import type { MobileConversationSelection } from "../conversation/mobile-conversation"
import type { MobileConversationThread } from "../conversation/mobile-conversation"
import type { FullAutoRunProjectionResult } from "../full-auto/full-auto-run-projection"
import type { FullAutoRunControlDispatchOutcome } from "../full-auto/full-auto-run-control-intent"
import type { FullAutoRunControlAction } from "@openagentsinc/khala-sync"
import type { SarahPrincipalProjection } from "@openagentsinc/sarah"
import type { ManagedSandboxSupervisionProjection } from "@openagentsinc/managed-sandbox-contract"
import type {
  MobileManagedSandboxControlAction,
  MobileManagedSandboxControlResult,
  MobileManagedSandboxSnapshot,
} from "../managed-sandbox/mobile-managed-sandbox"
import { EffectNativeHost } from "../effect-native/effect-native-host"
import {
  enableMobileLayoutAnimation,
  prepareMobileNativeIntentFeedback,
} from "../effect-native/mobile-native-feedback"
import { sendKhalaTurn } from "../khala/khala-client"
import { mobileWorkspaceKeyboardCommand } from "./mobile-workspace-keyboard"
import {
  buildHomeProgram,
  normalizeMobileAccessibilityProfile,
  renderHomeView,
  type MobileSyncPhase,
  type SarahSpeechPlaybackPort,
} from "./home-core"

type SarahPlaybackRecord = Readonly<{
  player: import("expo-audio").AudioPlayer
  subscription: Readonly<{ remove: () => void }>
  fileUri: string
  resolve: (outcome: "completed" | "stopped") => void
}>

/**
 * React Native is the capability host: safe-area and keyboard avoidance wrap
 * one Effect Native surface. The application tree, chrome, drawer, composer,
 * state, and actions stay in the typed view program.
 */
const enPlatform = Platform.OS === "android" ? ("android" as const) : ("ios" as const)

export const HomeScreen = ({
  syncPhase,
  sessionActions,
  conversation,
  coding,
  pendingAttentionTarget,
  onAttentionTargetConsumed,
  fullAutoRun,
  fullAutoControl,
  managedSandboxes,
  managedSandboxControl,
  sarah,
  sarahSpeech,
  notificationSettings,
  incomingShare,
  onShareConsumed,
}: {
  readonly syncPhase: MobileSyncPhase
  readonly sessionActions: Readonly<{
    signIn: () => Promise<void>
    signOut: () => Promise<void>
  }>
  readonly conversation?: Extract<MobileConversationSelection, { readonly mode: "sync" }>
  /** Live `FullAutoRun` mobile projection (openagents #8982); pushed into the
   * program on every change so the state header updates without a restart. */
  readonly fullAutoRun?: FullAutoRunProjectionResult | null
  /** MOB-FA-02 (#8994): dispatches a Pause/Resume/Stop control intent and
   * resolves once a durable applied/rejected/pending outcome is known.
   * Absent means Full Auto remote control is unavailable on this build. */
  readonly fullAutoControl?: (input: Readonly<{
    runRef: string
    action: FullAutoRunControlAction
  }>) => Promise<FullAutoRunControlDispatchOutcome>
  readonly managedSandboxes?: MobileManagedSandboxSnapshot | null
  readonly managedSandboxControl?: (input: Readonly<{
    projection: ManagedSandboxSupervisionProjection
    action: MobileManagedSandboxControlAction
  }>) => Promise<MobileManagedSandboxControlResult>
  readonly sarah?: SarahPrincipalProjection | null
  readonly sarahSpeech?: (input: Readonly<{
    threadRef: string
    messageRef: string
    text: string
  }>) => Promise<Readonly<
    | { state: "ready"; fileUri: string }
    | { state: "unauthorized" | "forbidden" | "too_long" | "unavailable"; message: string }
  >>
  readonly notificationSettings?: MobileNotificationSettingsPort
  readonly incomingShare?: MobileShareIntake | null
  readonly onShareConsumed?: () => void
  readonly coding?: Readonly<{
    directory: MobileCodingDirectory
    portableSnapshot: ConfirmedPortableSessionSnapshot | null
    attentionSnapshot: ConfirmedRuntimeAttentionSnapshot | null
    requestPortableAction: (input: Readonly<{
      sessionRef: string
      action: MobilePortableControlAction
      destinationTargetRef?: string
    }>) => Promise<Readonly<
      | { state: "queued"; snapshot: ConfirmedPortableSessionSnapshot }
      | { state: "rejected"; reason: MobilePortableUnavailableReason; snapshot: ConfirmedPortableSessionSnapshot | null }
    >>
    activeComposer: () => MobileCodingComposerSession | null
    executionTargets?: ReadonlyArray<MobileExecutionTargetOption>
    fleetRuns?: import("@openagentsinc/khala-sync").FleetRunClientProjection
    searchComposerPaths?: MobileComposerPathSearchPort["search"]
    repositoryFiles?: MobileRepositoryFilesPort
    repositoryReview?: MobileRepositoryReviewPort
    repositoryGit?: MobileRepositoryGitPort
    repositoryTerminal?: MobileRepositoryTerminalPort
    environmentConnections?: MobileEnvironmentConnectionsPort
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
    selectComposerTarget?: (
      session: MobileCodingComposerSession,
      target: MobileExecutionTargetOption,
    ) => Promise<MobileCodingComposerSession | null>
    pickComposerAttachments: (
      session: MobileCodingComposerSession,
    ) => Promise<MobileCodingAttachmentUpdateResult>
  }>
  readonly pendingAttentionTarget?: MobileAttentionTarget | null
  readonly onAttentionTargetConsumed?: () => void
}) => {
  const { fontScale, width } = useWindowDimensions()
  const initialWorkspaceWidth = useRef(width).current
  const [reduceMotion, setReduceMotion] = useState(false)
  const attentionDispatchRef = useRef<string | null>(null)
  const sarahPlaybackRef = useRef<SarahPlaybackRecord | null>(null)
  const releaseSarahPlayback = useCallback((outcome: "completed" | "stopped"): void => {
    const record = sarahPlaybackRef.current
    if (record === null) return
    sarahPlaybackRef.current = null
    record.subscription.remove()
    try { record.player.pause() } catch { /* already settled */ }
    try { record.player.remove() } catch { /* already released */ }
    try {
      const file = new File(record.fileUri)
      if (file.exists) file.delete()
    } catch { /* cache cleanup is best effort */ }
    record.resolve(outcome)
  }, [])
  const sarahSpeechPlayback = useMemo<SarahSpeechPlaybackPort | undefined>(() =>
    sarahSpeech === undefined
      ? undefined
      : {
          stop: () => releaseSarahPlayback("stopped"),
          play: async input => {
            releaseSarahPlayback("stopped")
            const result = await sarahSpeech(input)
            if (result.state !== "ready") {
              return { state: "unavailable" as const, message: result.message }
            }
            try {
              const { createAudioPlayer } = require("expo-audio") as typeof import("expo-audio")
              const player = createAudioPlayer(result.fileUri, { updateInterval: 100 })
              let settle!: (outcome: "completed" | "stopped") => void
              const completed = new Promise<"completed" | "stopped">(resolve => {
                settle = resolve
              })
              const subscription = player.addListener("playbackStatusUpdate", status => {
                if (status.didJustFinish) releaseSarahPlayback("completed")
              })
              sarahPlaybackRef.current = {
                player,
                subscription,
                fileUri: result.fileUri,
                resolve: settle,
              }
              player.play()
              return { state: "started" as const, completed }
            } catch {
              try {
                const file = new File(result.fileUri)
                if (file.exists) file.delete()
              } catch { /* cache cleanup is best effort */ }
              return {
                state: "unavailable" as const,
                message: "Sarah voice could not play on this device.",
              }
            }
          },
        },
    [sarahSpeech, releaseSarahPlayback],
  )
  const accessibility = useMemo(
    () => normalizeMobileAccessibilityProfile({ fontScale, reduceMotion }),
    [fontScale, reduceMotion],
  )
  const program = useMemo(
    () => buildHomeProgram({
      khalaTurn: { sendTurn: sendKhalaTurn },
      sessionActions,
      conversation,
      accessibility,
      workspaceWidth: initialWorkspaceWidth,
      coding,
      settings: {
        ...(coding?.environmentConnections === undefined ? {} : { environments: coding.environmentConnections }),
        ...(notificationSettings === undefined ? {} : { notifications: notificationSettings }),
        incomingShare: incomingShare ?? null,
        ...(onShareConsumed === undefined ? {} : { onShareConsumed }),
      },
      ...(fullAutoRun === null || fullAutoRun === undefined ? {} : { fullAutoRun }),
      ...(fullAutoControl === undefined ? {} : { fullAutoControl }),
      ...(managedSandboxes === null || managedSandboxes === undefined
        ? {}
        : { managedSandboxes }),
      ...(managedSandboxControl === undefined ? {} : { managedSandboxControl }),
      ...(sarah === null || sarah === undefined ? {} : { sarah }),
      ...(sarahSpeechPlayback === undefined ? {} : { sarahSpeech: sarahSpeechPlayback }),
    }),
    // fullAutoRun deliberately excluded: its initial value seeds the program
    // once at mount; later changes flow through `program.fullAuto.setProjection`
    // below (mirroring the syncPhase push pattern) so a live projection poll
    // never tears down and rebuilds the whole Effect Native program.
    // fullAutoControl is stable across the component's lifetime (a plain
    // capability closure over the sync host, not per-render state) the same
    // way `sessionActions` and `coding` are already treated below.
    [sessionActions, conversation, coding, notificationSettings, onShareConsumed, initialWorkspaceWidth, fullAutoControl, managedSandboxControl, sarah, sarahSpeechPlayback],
  )
  const report = useMemo<IntentReporter>(() => (ref, runtimeValue) => {
    prepareMobileNativeIntentFeedback(ref.name, accessibility.reduceMotion)
    return program.report(ref, runtimeValue)
  }, [program, accessibility.reduceMotion])
  useEffect(enableMobileLayoutAnimation, [])
  useEffect(() => {
    let active = true
    void AccessibilityInfo.isReduceMotionEnabled().then(enabled => {
      if (active) setReduceMotion(enabled)
    })
    const subscription = AccessibilityInfo.addEventListener(
      "reduceMotionChanged",
      setReduceMotion,
    )
    return () => {
      active = false
      subscription.remove()
    }
  }, [])
  useEffect(() => {
    program.accessibility.setProfile(accessibility)
  }, [program, accessibility])
  useEffect(() => {
    program.workspace.setWidth(width)
  }, [program, width])
  useEffect(() => () => {
    releaseSarahPlayback("stopped")
    void program.close()
  }, [program, releaseSarahPlayback])
  useEffect(() => {
    program.sync.setPhase(syncPhase)
  }, [program, syncPhase])
  useEffect(() => {
    program.fullAuto.setProjection(fullAutoRun ?? null)
  }, [program, fullAutoRun])
  useEffect(() => {
    program.managedSandboxes.setSnapshot(managedSandboxes ?? null)
  }, [program, managedSandboxes])
  useEffect(() => {
    program.settings.setIncomingShare(incomingShare ?? null)
  }, [program, incomingShare])
  useEffect(() => {
    const subscription = AppState.addEventListener("change", next => {
      if (next === "active") program.coding.recoverTerminal()
    })
    return () => subscription.remove()
  }, [program])
  useEffect(() => {
    if (pendingAttentionTarget === null || pendingAttentionTarget === undefined) return
    if (attentionDispatchRef.current === pendingAttentionTarget.attentionRef) return
    attentionDispatchRef.current = pendingAttentionTarget.attentionRef
    void program.controller.selectAttention(pendingAttentionTarget).finally(() => {
      attentionDispatchRef.current = null
      onAttentionTargetConsumed?.()
    })
  }, [program, pendingAttentionTarget, onAttentionTargetConsumed])

  return (
    <RNView
      style={{ flex: 1, backgroundColor: khalaTheme.color.background }}
      {...({
        onKeyDown: (event: Readonly<{ nativeEvent?: Readonly<{
          key?: string
          metaKey?: boolean
          ctrlKey?: boolean
        }> }>) => {
          const command = mobileWorkspaceKeyboardCommand(event.nativeEvent ?? {})
          if (command !== null) program.workspace.dispatchKeyboardCommand(command)
        },
      } as Record<string, unknown>)}
    >
      <KeyboardAvoidingView
        behavior={Platform.OS === "ios" ? "padding" : "height"}
        style={{ flex: 1 }}
      >
        <SafeAreaView edges={["top", "bottom"]} style={{ flex: 1 }}>
          <EffectNativeHost
            viewStream={program.viewStream}
            report={report}
            theme={khalaTheme}
            platform={enPlatform}
            initialView={renderHomeView({ ...program.initialState, accessibility })}
          />
        </SafeAreaView>
      </KeyboardAvoidingView>
    </RNView>
  )
}
