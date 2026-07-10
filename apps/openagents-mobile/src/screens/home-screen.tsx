import { useEvent } from "expo"
import { useVideoPlayer, VideoView } from "expo-video"
import { useEffect, useMemo, useRef, useState } from "react"
import {
  Animated,
  Easing,
  Platform,
  Pressable,
  Text as RNText,
  View as RNView,
} from "react-native"
import { SafeAreaView, useSafeAreaInsets } from "react-native-safe-area-context"

import { Effect, Stream } from "@effect-native/core/effect"
import { khalaTheme } from "@effect-native/tokens"

import {
  loadGlassComposer,
  loadGlassIconButton,
  loadGlassOptionSheet,
  loadGlassPill,
} from "openagents-liquid-glass"
import { EffectNativeHost } from "../effect-native/effect-native-host"
import {
  buildHomeProgram,
  chromeProps,
  initialHomeState,
  mineralPacks,
  renderContentView,
  renderDrawerView,
  surfaceModeOptions,
} from "./home-core"

// Bundled Sarah demo loop (assets/videos/sarah-demo.mp4, ~1.7 MB) — the
// "Sarah" surface mode plays it fullscreen UNDER the glass chrome.
// eslint-disable-next-line @typescript-eslint/no-require-imports
const sarahDemoVideo = require("../../assets/videos/sarah-demo.mp4") as number
// Composer-tap reply video (assets/videos/ask-anything.mp4, ~2.1 MB) — plays
// fullscreen WITH AUDIO on "Ask anything" tap (owner direction 2026-07-09);
// preloaded by creating its player at mount.
// eslint-disable-next-line @typescript-eslint/no-require-imports
const askAnythingVideo = require("../../assets/videos/ask-anything.mp4") as number

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
const GlassOptionSheet = Platform.OS === "ios" ? loadGlassOptionSheet() : undefined

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

  // Sarah surface-mode video: looping, muted, cover-fit, UNDER all glass.
  const sarahMode = homeState.surfaceMode === "sarah"
  const player = useVideoPlayer(sarahDemoVideo, (p) => {
    p.loop = true
    p.muted = true
  })
  const { status: videoStatus } = useEvent(player, "statusChange", {
    status: player.status,
  })
  const videoOpacity = useRef(new Animated.Value(0)).current
  const videoScale = useRef(new Animated.Value(1.03)).current

  // "Ask anything" takeover video — AUDIO ON (muted=false), no loop,
  // preloaded at mount; plays from the start on each composer tap and
  // dismisses on play-to-end or tap (typed AskVideoDismissed intent).
  const askPlaying = homeState.askVideoPlaying
  const askPlayer = useVideoPlayer(askAnythingVideo, (p) => {
    p.loop = false
    p.muted = false
  })
  useEffect(() => {
    if (askPlaying) {
      askPlayer.currentTime = 0
      askPlayer.play()
    } else {
      askPlayer.pause()
    }
  }, [askPlaying, askPlayer])
  useEffect(() => {
    const subscription = askPlayer.addListener("playToEnd", () => {
      // Video over -> takeover ends; the ORIGINAL surface (Sarah loop /
      // black) resumes underneath (owner direction).
      program.chrome.dismissAskVideo()
    })
    return () => {
      subscription.remove()
    }
  }, [askPlayer, program])

  // Midway through the ask video (4s of the 8s clip) the minerals sheet
  // flies up from the bottom (typed MineralsSheetOpened intent).
  useEffect(() => {
    if (!askPlaying) return
    const timer = setTimeout(() => {
      program.chrome.openMineralsSheet()
    }, 4000)
    return () => {
      clearTimeout(timer)
    }
  }, [askPlaying, program])

  // Fly-up animation for the minerals sheet (host machinery; content is the
  // SwiftUI glass island).
  const sheetOpen = homeState.mineralsSheetOpen
  const sheetY = useRef(new Animated.Value(420)).current
  useEffect(() => {
    if (sheetOpen) {
      sheetY.setValue(420)
      Animated.spring(sheetY, {
        toValue: 0,
        useNativeDriver: true,
        damping: 20,
        stiffness: 180,
        mass: 0.9,
      }).start()
    }
  }, [sheetOpen, sheetY])

  useEffect(() => {
    if (sarahMode) {
      player.play()
    } else {
      player.pause()
      // Reset so re-entering Sarah mode fades in again from black.
      videoOpacity.setValue(0)
      videoScale.setValue(1.03)
    }
  }, [sarahMode, player, videoOpacity, videoScale])

  // Fade-in on first-frame-ready — never a hard pop: opacity 0 -> 1 (~700ms
  // ease-out) with a subtle 1.03 -> 1.0 scale settle. Black underneath until
  // ready keeps the transition seamless.
  useEffect(() => {
    if (sarahMode && videoStatus === "readyToPlay") {
      Animated.parallel([
        Animated.timing(videoOpacity, {
          toValue: 1,
          duration: 700,
          easing: Easing.out(Easing.cubic),
          useNativeDriver: true,
        }),
        Animated.timing(videoScale, {
          toValue: 1,
          duration: 900,
          easing: Easing.out(Easing.cubic),
          useNativeDriver: true,
        }),
      ]).start()
    }
  }, [sarahMode, videoStatus, videoOpacity, videoScale])

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
    <RNView style={{ flex: 1, backgroundColor: khalaTheme.color.background }}>
      {/* 0. Sarah demo video — fullscreen cover, looping, muted, faded in on
          first-frame-ready, UNDER every glass layer. Black shows through
          until ready (and always in OpenAgents mode). */}
      {sarahMode ? (
        <Animated.View
          style={{
            position: "absolute",
            top: 0,
            bottom: 0,
            left: 0,
            right: 0,
            opacity: videoOpacity,
            transform: [{ scale: videoScale }],
          }}
        >
          <VideoView
            player={player}
            style={{ flex: 1 }}
            contentFit="cover"
            nativeControls={false}
          />
        </Animated.View>
      ) : null}
      {/* 0b. "Ask anything" reply video — WITH audio, UNDER the chrome
          (same layering as the Sarah loop, owner direction); tap outside the
          chrome dismisses; auto-dismiss on play-to-end resumes the original
          surface underneath. */}
      {askPlaying ? (
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Dismiss video"
          onPress={program.chrome.dismissAskVideo}
          style={{
            position: "absolute",
            top: 0,
            bottom: 0,
            left: 0,
            right: 0,
            backgroundColor: khalaTheme.color.background,
          }}
        >
          <VideoView
            player={askPlayer}
            style={{ flex: 1 }}
            contentFit="cover"
            nativeControls={false}
          />
        </Pressable>
      ) : null}
      <SafeAreaView edges={["top"]} style={{ flex: 1 }} pointerEvents="box-none">
      {/* 1. EN content surface (touch-transparent while the ask video plays
          so taps reach the video's dismiss Pressable; the EN surface is
          content-empty by owner direction, so nothing loses interactivity) */}
      <RNView style={{ flex: 1 }} pointerEvents={askPlaying ? "none" : "auto"}>
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
                accessibilityLabel={chrome.pillLabel}
                // Fallback dropdown: tapping cycles the surface mode.
                onPress={() =>
                  program.chrome.selectSurfaceMode(sarahMode ? "openagents" : "sarah")
                }
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
                options={surfaceModeOptions}
                selectedId={chrome.surfaceMode}
                onSelect={(event) =>
                  program.chrome.selectSurfaceMode(
                    event.nativeEvent.id === "sarah" ? "sarah" : "openagents",
                  )
                }
                onTap={program.chrome.pressPill}
                // Hug the content: the SwiftUI capsule centers inside the RN
                // frame, so a fixed 180pt frame made short labels ("Sarah")
                // look right-aligned. Width tracks the label (owner fix,
                // shipped OTA).
                style={{ width: 76 + chrome.pillLabel.length * 10, height: 44 }}
              />
            )}
            <RNView style={{ flex: 1 }} />
            {/* Search icon removed from the chrome (owner direction
                2026-07-09); search stays reachable in the drawer. */}
            {GlassIconButton === undefined ? null : (
              <GlassIconButton
                symbol="square.and.pencil"
                accessibilityLabelText="New chat"
                onTap={program.chrome.pressNewChat}
                style={{ width: 44, height: 44 }}
              />
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
      {/* 4. Minerals fly-up sheet — Liquid Glass panel over the bottom
          third; opened midway through the ask video; options dispatch typed
          MineralPackSelected / MineralsSheetDismissed intents. */}
      {sheetOpen ? (
        <Animated.View
          style={{
            position: "absolute",
            left: 8,
            right: 8,
            bottom: insets.bottom + 8,
            height: 340,
            transform: [{ translateY: sheetY }],
          }}
        >
          {GlassOptionSheet === undefined ? (
            <RNView
              style={{
                flex: 1,
                borderRadius: 28,
                padding: 16,
                gap: 8,
                ...fallbackChromeStyle,
              }}
            >
              <RNText
                style={{
                  color: khalaTheme.color.textPrimary,
                  fontWeight: "700",
                  textAlign: "center",
                }}
              >
                Buy Minerals
              </RNText>
              {mineralPacks.map((pack) => (
                <Pressable
                  key={pack.id}
                  accessibilityRole="button"
                  accessibilityLabel={pack.label}
                  onPress={() => program.chrome.selectMineralPack(pack.id)}
                  style={{
                    height: 44,
                    borderRadius: 14,
                    paddingHorizontal: 16,
                    flexDirection: "row",
                    alignItems: "center",
                    justifyContent: "space-between",
                    backgroundColor: "rgba(255, 255, 255, 0.07)",
                  }}
                >
                  <RNText style={{ color: khalaTheme.color.textPrimary, fontWeight: "600" }}>
                    {pack.label}
                  </RNText>
                  <RNText style={{ color: khalaTheme.color.accent, fontWeight: "600" }}>
                    {pack.price}
                  </RNText>
                </Pressable>
              ))}
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Not now"
                onPress={program.chrome.dismissMineralsSheet}
                style={{ height: 36, alignItems: "center", justifyContent: "center" }}
              >
                <RNText style={{ color: khalaTheme.color.textMuted }}>Not now</RNText>
              </Pressable>
            </RNView>
          ) : (
            <GlassOptionSheet
              title="Buy Minerals"
              options={mineralPacks}
              onSelect={(event) => program.chrome.selectMineralPack(event.nativeEvent.id)}
              onDismiss={program.chrome.dismissMineralsSheet}
              style={{ flex: 1 }}
            />
          )}
        </Animated.View>
      ) : null}
    </RNView>
  )
}
