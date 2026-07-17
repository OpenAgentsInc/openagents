import { useEffect, useMemo, useState } from "react"
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
import type {
  ConfirmedPortableSessionSnapshot,
} from "@openagentsinc/khala-sync-client"
import type {
  MobilePortableControlAction,
  MobilePortableUnavailableReason,
} from "../coding/mobile-portable-session-controls"
import type { MobileConversationSelection } from "../conversation/mobile-conversation"
import type { MobileConversationThread } from "../conversation/mobile-conversation"
import { EffectNativeHost } from "../effect-native/effect-native-host"
import { sendKhalaTurn } from "../khala/khala-client"
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

export const HomeScreen = ({ syncPhase, sessionActions, conversation, coding }: {
  readonly syncPhase: MobileSyncPhase
  readonly sessionActions: Readonly<{
    signIn: () => Promise<void>
    signOut: () => Promise<void>
  }>
  readonly conversation?: Extract<MobileConversationSelection, { readonly mode: "sync" }>
  readonly coding?: Readonly<{
    directory: MobileCodingDirectory
    portableSnapshot: ConfirmedPortableSessionSnapshot | null
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
}) => {
  const { fontScale } = useWindowDimensions()
  const [reduceMotion, setReduceMotion] = useState(false)
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
      coding,
    }),
    [sessionActions, conversation, coding],
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
    program.sync.setPhase(syncPhase)
  }, [program, syncPhase])

  return (
    <RNView style={{ flex: 1, backgroundColor: khalaTheme.color.background }}>
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
