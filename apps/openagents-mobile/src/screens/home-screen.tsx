import { useEffect, useMemo, useRef, useState } from "react"
import {
  AccessibilityInfo,
  KeyboardAvoidingView,
  Platform,
  View as RNView,
  useWindowDimensions,
} from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { khalaTheme } from "@effect-native/tokens"

import type { MobileCodingDirectory, MobileCodingTarget } from "../coding/mobile-coding-navigation"
import type {
  MobileCodingAttachmentUpdateResult,
  MobileCodingComposerSession,
} from "../coding/mobile-coding-composer"
import type { MobileExecutionTargetOption } from "../coding/mobile-execution-targets"
import type { MobileComposerPathSearchPort } from "../coding/mobile-composer-path-context"
import type { MobileRepositoryFilesPort } from "../coding/mobile-repository-files"
import type { MobileRepositoryReviewPort } from "../coding/mobile-repository-review"
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
import { EffectNativeHost } from "../effect-native/effect-native-host"
import { sendKhalaTurn } from "../khala/khala-client"
import { mobileWorkspaceKeyboardCommand } from "./mobile-workspace-keyboard"
import {
  buildHomeProgram,
  normalizeMobileAccessibilityProfile,
  renderHomeView,
  type MobileSyncPhase,
} from "./home-core"

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
      ...(fullAutoRun === null || fullAutoRun === undefined ? {} : { fullAutoRun }),
    }),
    // fullAutoRun deliberately excluded: its initial value seeds the program
    // once at mount; later changes flow through `program.fullAuto.setProjection`
    // below (mirroring the syncPhase push pattern) so a live projection poll
    // never tears down and rebuilds the whole Effect Native program.
    [sessionActions, conversation, coding, initialWorkspaceWidth],
  )
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
    void program.close()
  }, [program])
  useEffect(() => {
    program.sync.setPhase(syncPhase)
  }, [program, syncPhase])
  useEffect(() => {
    program.fullAuto.setProjection(fullAutoRun ?? null)
  }, [program, fullAutoRun])
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
            report={program.report}
            theme={khalaTheme}
            platform={enPlatform}
            initialView={renderHomeView({ ...program.initialState, accessibility })}
          />
        </SafeAreaView>
      </KeyboardAvoidingView>
    </RNView>
  )
}
