import { useEffect, useMemo, useState } from "react"
import { Platform, Pressable, Text as RNText, View as RNView } from "react-native"
import { SafeAreaView, useSafeAreaInsets } from "react-native-safe-area-context"

import { Effect, Stream } from "@effect-native/core/effect"
import { khalaTheme } from "@effect-native/tokens"

import {
  loadGlassComposer,
  loadGlassIconButton,
  loadGlassPill,
} from "openagents-liquid-glass"
import { EffectNativeHost } from "../effect-native/effect-native-host"
import {
  buildHomeProgram,
  chromeProps,
  initialHomeState,
  renderContentView,
  renderDrawerView,
} from "./home-core"

/**
 * OpenAgents mobile (GL-2 #8648, #8597) — the ChatGPT-style glass shell.
 *
 * Layering (host machinery only — the v26 typed style system deliberately has
 * no absolute positioning, so the SHELL owns z-order; the EN program owns
 * every tree and every intent):
 *
 *   1. EN content surface (fills the screen).
 *   2. Floating glass chrome — SwiftUI Liquid Glass islands (iOS 26
 *      .glassEffect, material fallback pre-26): top row = sidebar toggle,
 *      OpenAgents pill, search; bottom = composer bar with plus/mic. Hidden
 *      while the drawer is open. Every tap dispatches a typed intent through
 *      `program.chrome` (the ONLY seam native events enter the program).
 *   3. Drawer overlay when `drawerOpen`: scrim (tap = DrawerToggled) + the EN
 *      drawer panel (a SECOND view projection of the SAME program state).
 *
 * Android / Expo Go render honest RN fallbacks for the chrome (no fake glass).
 */
const GlassIconButton = Platform.OS === "ios" ? loadGlassIconButton() : undefined
const GlassPill = Platform.OS === "ios" ? loadGlassPill() : undefined
const GlassComposer = Platform.OS === "ios" ? loadGlassComposer() : undefined

const enPlatform = Platform.OS === "android" ? ("android" as const) : ("ios" as const)

const fallbackChromeStyle = {
  backgroundColor: "rgba(11, 18, 32, 0.9)",
  borderColor: khalaTheme.color.border,
  borderWidth: 1,
} as const

export const HomeScreen = () => {
  // Build the program (state ref + intent registry + two view projections)
  // once per mount.
  const program = useMemo(buildHomeProgram, [])
  const [homeState, setHomeState] = useState(initialHomeState)
  const insets = useSafeAreaInsets()

  // Mirror program state into React state for chrome props/visibility — one
  // source of truth (the program's SubscriptionRef), many renderers reading it.
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

  const chrome = chromeProps(homeState)

  return (
    <SafeAreaView
      edges={["top"]}
      style={{ flex: 1, backgroundColor: khalaTheme.color.background }}
    >
      {/* 1. EN content surface */}
      <RNView style={{ flex: 1 }}>
        <EffectNativeHost
          viewStream={program.contentViewStream}
          report={program.report}
          theme={khalaTheme}
          platform={enPlatform}
          initialView={renderContentView(initialHomeState)}
        />
      </RNView>

      {/* 2. Floating glass chrome (hidden while the drawer is open) */}
      {chrome.chromeVisible ? (
        <>
          <RNView
            pointerEvents="box-none"
            style={{
              position: "absolute",
              top: insets.top + 8,
              left: 16,
              right: 16,
              flexDirection: "row",
              alignItems: "center",
              gap: 10,
            }}
          >
            {GlassIconButton === undefined ? (
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Open navigation"
                onPress={program.chrome.toggleDrawer}
                style={{
                  width: 44,
                  height: 44,
                  borderRadius: 22,
                  alignItems: "center",
                  justifyContent: "center",
                  ...fallbackChromeStyle,
                }}
              >
                <RNText style={{ color: khalaTheme.color.textPrimary, fontSize: 18 }}>≡</RNText>
              </Pressable>
            ) : (
              <GlassIconButton
                symbol="line.3.horizontal"
                accessibilityLabelText="Open navigation"
                onTap={program.chrome.toggleDrawer}
                style={{ width: 44, height: 44 }}
              />
            )}
            {GlassPill === undefined ? (
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="OpenAgents"
                onPress={program.chrome.pressPill}
                style={{
                  height: 44,
                  borderRadius: 22,
                  paddingHorizontal: 16,
                  justifyContent: "center",
                  ...fallbackChromeStyle,
                }}
              >
                <RNText style={{ color: khalaTheme.color.textPrimary, fontWeight: "600" }}>
                  {chrome.pillLabel}
                </RNText>
              </Pressable>
            ) : (
              <GlassPill
                label={chrome.pillLabel}
                symbol="sparkles"
                onTap={program.chrome.pressPill}
                style={{ width: 168, height: 44 }}
              />
            )}
            <RNView style={{ flex: 1 }} />
            {GlassIconButton === undefined ? (
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Search"
                onPress={program.chrome.pressSearch}
                style={{
                  width: 44,
                  height: 44,
                  borderRadius: 22,
                  alignItems: "center",
                  justifyContent: "center",
                  ...fallbackChromeStyle,
                }}
              >
                <RNText style={{ color: khalaTheme.color.textPrimary, fontSize: 16 }}>?</RNText>
              </Pressable>
            ) : (
              <>
                <GlassIconButton
                  symbol="magnifyingglass"
                  accessibilityLabelText="Search"
                  onTap={program.chrome.pressSearch}
                  style={{ width: 44, height: 44 }}
                />
                <GlassIconButton
                  symbol="square.and.pencil"
                  accessibilityLabelText="New chat"
                  onTap={program.chrome.pressNewChat}
                  style={{ width: 44, height: 44 }}
                />
              </>
            )}
          </RNView>

          <RNView
            pointerEvents="box-none"
            style={{
              position: "absolute",
              bottom: insets.bottom + 16,
              left: 0,
              right: 0,
            }}
          >
            {GlassComposer === undefined ? (
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Composer"
                onPress={program.chrome.pressComposer}
                style={{
                  marginHorizontal: 16,
                  height: 54,
                  borderRadius: 27,
                  paddingHorizontal: 18,
                  justifyContent: "center",
                  ...fallbackChromeStyle,
                }}
              >
                <RNText style={{ color: khalaTheme.color.textMuted }}>
                  {chrome.composerPlaceholder}
                </RNText>
              </Pressable>
            ) : (
              <GlassComposer
                placeholder={chrome.composerPlaceholder}
                onTapComposer={program.chrome.pressComposer}
                onTapMic={program.chrome.pressMic}
                onTapPlus={program.chrome.pressNewChat}
                style={{ height: 54 }}
              />
            )}
          </RNView>
        </>
      ) : null}

      {/* 3. Drawer overlay — scrim + the EN drawer panel projection */}
      {homeState.drawerOpen ? (
        <RNView
          style={{
            position: "absolute",
            top: 0,
            bottom: 0,
            left: 0,
            right: 0,
            flexDirection: "row",
          }}
        >
          <RNView
            style={{
              width: "82%",
              height: "100%",
              borderRightWidth: 1,
              borderRightColor: khalaTheme.color.border,
            }}
          >
            <EffectNativeHost
              viewStream={program.drawerViewStream}
              report={program.report}
              theme={khalaTheme}
              platform={enPlatform}
              initialView={renderDrawerView(homeState)}
            />
          </RNView>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Close navigation"
            onPress={program.chrome.toggleDrawer}
            style={{ flex: 1, backgroundColor: "rgba(0, 0, 0, 0.55)" }}
          />
        </RNView>
      ) : null}
    </SafeAreaView>
  )
}
