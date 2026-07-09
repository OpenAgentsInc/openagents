import { useMemo } from "react"
import { Platform } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { khalaTheme } from "@effect-native/tokens"

import { EffectNativeHost } from "../effect-native/effect-native-host"
import {
  buildHomeProgram,
  initialHomeState,
  renderHomeView,
} from "./home-core"

/**
 * OpenAgents mobile (#8597) — the Home screen. Its UI is the typed `View`
 * program in `home-core.ts` rendered by `@effect-native/render-rn`; only this
 * thin outer shell is hand-written RN, to host the Effect Native surface
 * inside the Expo app. Theme: the shared Protoss-blue `khalaTheme` from
 * `@effect-native/tokens` — the same theme value every other Effect Native
 * host mounts.
 */
export const HomeScreen = () => {
  // Build the program (state ref + intent registry + reporter) once per mount.
  const program = useMemo(buildHomeProgram, [])

  return (
    <SafeAreaView
      edges={["top"]}
      style={{ flex: 1, backgroundColor: khalaTheme.color.background }}
    >
      <EffectNativeHost
        viewStream={program.viewStream}
        report={program.report}
        theme={khalaTheme}
        platform={Platform.OS === "android" ? "android" : "ios"}
        initialView={renderHomeView(initialHomeState)}
      />
    </SafeAreaView>
  )
}
