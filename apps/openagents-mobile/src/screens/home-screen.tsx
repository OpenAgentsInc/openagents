import { useEffect, useMemo } from "react"
import {
  KeyboardAvoidingView,
  Platform,
  View as RNView,
} from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { khalaTheme } from "@effect-native/tokens"

import type { MobileConversationSelection } from "../conversation/mobile-conversation"
import { EffectNativeHost } from "../effect-native/effect-native-host"
import { sendKhalaTurn } from "../khala/khala-client"
import {
  buildHomeProgram,
  renderHomeView,
  type MobileSyncPhase,
} from "./home-core"

/**
 * React Native is the capability host: safe-area and keyboard avoidance wrap
 * one Effect Native surface. The application tree, chrome, drawer, composer,
 * state, and actions stay in the typed view program.
 */
const enPlatform = Platform.OS === "android" ? ("android" as const) : ("ios" as const)

export const HomeScreen = ({ syncPhase, sessionActions, conversation }: {
  readonly syncPhase: MobileSyncPhase
  readonly sessionActions: Readonly<{
    signIn: () => Promise<void>
    signOut: () => Promise<void>
  }>
  readonly conversation?: Extract<MobileConversationSelection, { readonly mode: "sync" }>
}) => {
  const program = useMemo(
    () => buildHomeProgram({ khalaTurn: { sendTurn: sendKhalaTurn }, sessionActions, conversation }),
    [sessionActions, conversation],
  )
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
            initialView={renderHomeView(program.initialState)}
          />
        </SafeAreaView>
      </KeyboardAvoidingView>
    </RNView>
  )
}
