import { useEffect, useMemo, useState } from "react"
import { Platform, Text as RNText, View as RNView } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { Effect, Stream } from "@effect-native/core/effect"
import { khalaTheme } from "@effect-native/tokens"

import { loadLiquidGlassView } from "openagents-liquid-glass"
import { EffectNativeHost } from "../effect-native/effect-native-host"
import {
  buildHomeProgram,
  glassIslandProps,
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
 *
 * SwiftUI seam test (audit
 * docs/effect-native/2026-07-09-effect-native-swiftui-renderer-audit.md):
 * below the Effect Native surface the shell mounts the SwiftUI "Liquid Glass"
 * island at a per-component UIHostingController boundary (the audit's interop
 * case 2 — the catalog's closed `hostKinds` registry has no SwiftUI kind yet;
 * demand register row D-MB-02). The island is NOT a parallel component
 * system: its props are a pure projection of the SAME program state
 * (`glassIslandProps`), and its tap event dispatches the typed `GlassPinged`
 * intent through the SAME reporter seam the RN renderer uses — SwiftUI tap ->
 * typed intent -> state -> both the EN tree and the island re-render.
 */
const LiquidGlassView = Platform.OS === "ios" ? loadLiquidGlassView() : undefined

export const HomeScreen = () => {
  // Build the program (state ref + intent registry + reporter) once per mount.
  const program = useMemo(buildHomeProgram, [])
  const [homeState, setHomeState] = useState(initialHomeState)

  // Mirror program state into React state for the island props — one source
  // of truth (the program's SubscriptionRef), two renderers reading it.
  useEffect(() => {
    const controller = new AbortController()
    Effect.runPromise(
      Stream.runForEach(program.stateChanges, (next) =>
        Effect.sync(() => setHomeState(next)),
      ),
      { signal: controller.signal },
    ).catch(() => {
      // Interrupt on unmount is the expected exit; never crash the shell.
    })
    return () => {
      controller.abort()
    }
  }, [program])

  const island = glassIslandProps(homeState)

  return (
    <SafeAreaView
      edges={["top"]}
      style={{ flex: 1, backgroundColor: khalaTheme.color.background }}
    >
      {/* The EN root Stack is height:"full" (100% of its parent), so it MUST
          live inside a flex:1 wrapper. Rendering it directly under the
          SafeAreaView made "full" mean the whole safe area, which pushed the
          SwiftUI island below the fold — the build-105 invisible-island bug
          (owner escalation; simulator-reproduced before this fix). */}
      <RNView style={{ flex: 1 }}>
        <EffectNativeHost
          viewStream={program.viewStream}
          report={program.report}
          theme={khalaTheme}
          platform={Platform.OS === "android" ? "android" : "ios"}
          initialView={renderHomeView(initialHomeState)}
        />
      </RNView>
      {LiquidGlassView === undefined ? (
        <RNView
          style={{
            padding: 16,
            borderTopWidth: 1,
            borderTopColor: khalaTheme.color.border,
          }}
        >
          <RNText style={{ color: khalaTheme.color.textMuted, fontSize: 13 }}>
            SwiftUI Liquid Glass island unavailable in this host (requires the
            iOS native build — not Expo Go/Android).
          </RNText>
        </RNView>
      ) : (
        <LiquidGlassView
          title={island.title}
          subtitle={island.subtitle}
          buttonLabel={island.buttonLabel}
          tapCount={island.tapCount}
          onGlassTap={program.dispatchGlassTap}
          style={{ height: 220 }}
        />
      )}
    </SafeAreaView>
  )
}
