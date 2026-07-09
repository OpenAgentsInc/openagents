import { useMemo } from "react"
import { SafeAreaView } from "react-native-safe-area-context"

import { EffectNativeHost } from "../effect-native/effect-native-host"
import { khalaEffectNativeTheme } from "../effect-native/khala-effect-native-theme"
import { khalaMobileTheme } from "../theme/tokens"
import {
  buildAboutProgram,
  initialAboutState,
  renderAboutView,
} from "./about-effect-native-core"

/**
 * EN-3 (#8568) — the first Khala-mobile screen authored with the Effect Native
 * component set instead of hand-written RN/khala-mobile primitives. Its INTERNAL
 * UI is a typed `View` tree (see `about-effect-native-core.ts`) rendered by
 * `@effect-native/render-rn`; only this thin outer shell is hand-written RN, to
 * host the Effect Native surface inside the existing Expo/React-Navigation app.
 *
 * New-screens policy (per the issue): from here, new/changed mobile screens
 * author the component set through this adapter; existing screens migrate on
 * touch only, post-P0.
 */
export const AboutEffectNativeScreen = () => {
  // Build the program (state ref + intent registry + reporter) once per mount.
  const program = useMemo(buildAboutProgram, [])

  return (
    <SafeAreaView
      edges={["top"]}
      style={{ flex: 1, backgroundColor: khalaMobileTheme.background }}
    >
      <EffectNativeHost
        viewStream={program.viewStream}
        report={program.report}
        theme={khalaEffectNativeTheme}
        platform="ios"
        initialView={renderAboutView(initialAboutState)}
      />
    </SafeAreaView>
  )
}
