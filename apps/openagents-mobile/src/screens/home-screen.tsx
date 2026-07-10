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
  loadGlassPill,
} from "openagents-liquid-glass"
import { EffectNativeHost } from "../effect-native/effect-native-host"
import { sendKhalaTurn } from "../khala/khala-client"
import {
  loadPersistedSarahSession,
  loadPersistedSarahThreads,
  mintSarahProspectSession,
  persistSarahSession,
  persistSarahThread,
  runSarahEventStream,
  sendSarahTurn,
} from "../sarah/sarah-client"
import {
  buildHomeProgram,
  chromeProps,
  initialHomeState,
  renderContentView,
  renderDrawerView,
  renderMineralsSheetView,
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
 *   2. Floating glass chrome — SwiftUI Liquid Glass islands on iOS 26+ with
 *      ultra-thin-material fallbacks. State enters as serializable projections
 *      and every native action exits through the typed intent registry.
 *   3. Drawer overlay when `drawerOpen`: scrim (tap = DrawerToggled) + the EN
 *      drawer panel (a SECOND view projection of the SAME program state).
 */
const enPlatform = Platform.OS === "android" ? ("android" as const) : ("ios" as const)
const GlassIconButton = Platform.OS === "ios" ? loadGlassIconButton() : undefined
const GlassPill = Platform.OS === "ios" ? loadGlassPill() : undefined
const GlassComposer = Platform.OS === "ios" ? loadGlassComposer() : undefined

const fallbackChromeStyle = {
  backgroundColor: "rgba(11, 18, 32, 0.9)",
  borderColor: khalaTheme.color.border,
  borderWidth: 1,
} as const

export const HomeScreen = () => {
  // Build the program (state ref + intent registry + two view projections)
  // once per mount. The production Sarah turn client is the ONE injected
  // effect seam (GL-3 #8649); everything else reaches the program as typed
  // intents.
  const program = useMemo(
    () =>
      buildHomeProgram({
        sarahTurn: { sendTurn: sendSarahTurn },
        khalaTurn: { sendTurn: sendKhalaTurn },
      }),
    [],
  )
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
  // preloaded at mount; plays from the start on each composer tap. Ends on
  // play-to-end (typed AskVideoEnded playback event) or user tap (typed
  // AskVideoDismissed intent). Neither touches the minerals sheet — its
  // lifecycle is user-owned (owner P0, build 111 feedback).
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
      // black) resumes underneath (owner direction). This is a PLAYBACK
      // event, not a user intent: it must NEVER close the minerals sheet
      // (owner P0, build 111 feedback 2026-07-09) — if the sheet is open it
      // stays open over the resumed surface until the user picks a pack or
      // taps "Not now".
      program.chrome.askVideoEnded()
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

  // The drawer is a real, bounded local catalog — no seeded fake chats. It
  // contains only conversations this app has persisted, newest first.
  useEffect(() => {
    let cancelled = false
    void loadPersistedSarahThreads().then((threads) => {
      if (!cancelled) {
        program.recents.hydrate(threads.map((thread) => ({ id: thread.threadId, title: thread.title })))
      }
    })
    return () => {
      cancelled = true
    }
  }, [program])

  // --- GL-3 (#8649): Sarah session boot ------------------------------------
  // Entering Sarah mode restores the persisted prospect relationship from
  // disk (survives restarts) or mints one against production; failure lands
  // as the typed unavailable card — the composer never dies (a later send
  // can bootstrap the session through the turn itself).
  const sarahPhase = homeState.sarah.phase
  const activeRecentId = homeState.activeRecentId
  const conversationSource = homeState.conversationSource
  useEffect(() => {
    if (!sarahMode || sarahPhase !== "idle") return
    let cancelled = false
    void (async () => {
      try {
        if (conversationSource === "recent" && activeRecentId !== undefined) {
          const selected = (await loadPersistedSarahThreads()).find(
            (thread) => thread.threadId === activeRecentId,
          )
          if (cancelled) return
          if (selected !== undefined) {
            program.sarah.sessionReady({
              prospectRef: selected.prospectRef,
              threadId: selected.threadId,
              restored: true,
              entries: selected.entries,
            })
            return
          }
        }
        if (conversationSource === "new") {
          const minted = await mintSarahProspectSession()
          if (!cancelled) program.sarah.sessionReady({ ...minted, restored: false, entries: [] })
          return
        }
        const persisted = await loadPersistedSarahSession()
        if (cancelled) return
        if (persisted !== null) {
          program.sarah.sessionReady({
            prospectRef: persisted.prospectRef,
            threadId: persisted.threadId,
            restored: true,
            entries: persisted.entries,
          })
          return
        }
        const minted = await mintSarahProspectSession()
        if (cancelled) return
        program.sarah.sessionReady({ ...minted, restored: false, entries: [] })
      } catch {
        if (!cancelled) program.sarah.sessionUnavailable("session_mint_failed")
      }
    })()
    return () => {
      cancelled = true
    }
  }, [sarahMode, sarahPhase, activeRecentId, conversationSource, program])

  // Bounded SSE transcript/card stream with typed reconnect — runs while the
  // Sarah surface is active and a prospect relationship exists; aborted on
  // mode exit/unmount. (Verified server contract: the bus carries avatar-tier
  // events; text turns render from the POST reply — web-parity.)
  const sarahProspectRef = homeState.sarah.prospectRef
  useEffect(() => {
    if (!sarahMode || sarahProspectRef === null) return
    const controller = new AbortController()
    void runSarahEventStream({
      prospectRef: sarahProspectRef,
      signal: controller.signal,
      callbacks: {
        onStatus: (phase) => program.sarah.streamStatus(phase),
        onEvent: (event) => program.sarah.eventReceived(event),
      },
    }).catch(() => {
      // Abort on exit is the expected path; never crash the shell.
    })
    return () => {
      controller.abort()
      program.sarah.streamStatus("idle")
    }
  }, [sarahMode, sarahProspectRef, program])

  // Debounced session persistence: the relationship (ref + bounded settled
  // transcript) survives app restarts.
  const sarahThreadId = homeState.sarah.threadId
  const sarahEntries = homeState.sarah.entries
  useEffect(() => {
    if (sarahProspectRef === null || sarahThreadId === null) return
    const timer = setTimeout(() => {
      void persistSarahSession({
        prospectRef: sarahProspectRef,
        threadId: sarahThreadId,
        entries: sarahEntries
          .filter((entry) => entry.status === "done")
          .map((entry) => ({ key: entry.key, role: entry.role, text: entry.text })),
      })
    }, 400)
    return () => {
      clearTimeout(timer)
    }
  }, [sarahProspectRef, sarahThreadId, sarahEntries])

  // Keep the current conversation in the five-item catalog once it has a
  // real settled turn. The title is derived from the user's own first message
  // rather than fabricated UI copy.
  useEffect(() => {
    if (sarahProspectRef === null || sarahThreadId === null) return
    const settled = sarahEntries
      .filter((entry) => entry.status === "done")
      .map((entry) => ({ key: entry.key, role: entry.role, text: entry.text }))
    const firstUser = settled.find((entry) => entry.role === "user")
    if (firstUser === undefined) return
    let cancelled = false
    void persistSarahThread({
      prospectRef: sarahProspectRef,
      threadId: sarahThreadId,
      title: firstUser.text,
      updatedAt: Date.now(),
      entries: settled,
    }).then((threads) => {
      if (!cancelled) {
        program.recents.hydrate(threads.map((thread) => ({ id: thread.threadId, title: thread.title })))
      }
    })
    return () => {
      cancelled = true
    }
  }, [sarahProspectRef, sarahThreadId, sarahEntries, program])

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
          chrome dismisses; play-to-end resumes the original surface
          underneath. Neither ever closes the minerals sheet (layer 4). */}
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
              <Pressable accessibilityRole="button" accessibilityLabel="Open navigation" onPress={program.chrome.toggleDrawer} style={{ width: 44, height: 44, borderRadius: 22, alignItems: "center", justifyContent: "center", ...fallbackChromeStyle }}>
                <RNText style={{ color: khalaTheme.color.textPrimary, fontSize: 18 }}>≡</RNText>
              </Pressable>
            ) : <GlassIconButton symbol="line.3.horizontal" accessibilityLabelText="Open navigation" onTap={program.chrome.toggleDrawer} style={{ width: 44, height: 44 }} />}
            {GlassPill === undefined ? (
              <Pressable accessibilityRole="button" accessibilityLabel={chrome.pillLabel} onPress={() => program.chrome.selectSurfaceMode(homeState.surfaceMode === "openagents" ? "khala" : "openagents")} style={{ height: 44, borderRadius: 22, paddingHorizontal: 16, justifyContent: "center", ...fallbackChromeStyle }}>
                <RNText style={{ color: khalaTheme.color.textPrimary, fontWeight: "600" }}>{chrome.pillLabel}</RNText>
              </Pressable>
            ) : <GlassPill label={chrome.pillLabel} symbol="sparkles" options={surfaceModeOptions} selectedId={chrome.surfaceMode} onSelect={(event) => program.chrome.selectSurfaceMode(event.nativeEvent.id === "sarah" ? "sarah" : event.nativeEvent.id === "khala" ? "khala" : "openagents")} style={{ width: 76 + chrome.pillLabel.length * 10, height: 44 }} />}
            <RNView style={{ flex: 1 }} />
            {GlassIconButton === undefined ? null : <GlassIconButton symbol="square.and.pencil" accessibilityLabelText="New chat" onTap={program.chrome.pressNewChat} style={{ width: 44, height: 44 }} />}
          </RNView>

          {/* GL-3: the tap-only glass composer belongs to the demo surface;
              in Sarah mode the EN Composer (a real text input inside the
              conversation surface) replaces it. */}
          {chrome.glassComposerVisible ? (
          <RNView
            pointerEvents="box-none"
            style={{
              position: "absolute",
              bottom: insets.bottom + 16,
              left: 16,
              right: 16,
            }}
          >
            {GlassComposer === undefined ? (
              <Pressable accessibilityRole="button" accessibilityLabel="Composer" onPress={program.chrome.pressComposer} style={{ marginHorizontal: 16, height: 54, borderRadius: 27, paddingHorizontal: 18, justifyContent: "center", ...fallbackChromeStyle }}>
                <RNText style={{ color: khalaTheme.color.textMuted }}>{chrome.composerPlaceholder}</RNText>
              </Pressable>
            ) : <GlassComposer placeholder={chrome.composerPlaceholder} onTapComposer={program.chrome.pressComposer} onTapMic={program.chrome.pressMic} onTapPlus={program.chrome.pressNewChat} style={{ height: 54 }} />}
          </RNView>
          ) : null}
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
          MineralPackSelected / MineralsSheetDismissed intents. Those USER
          intents are the ONLY way it closes — it survives the video's
          play-to-end/loop boundary and stays over the resumed surface. */}
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
          <EffectNativeHost
            viewStream={program.mineralsSheetViewStream}
            report={program.report}
            theme={khalaTheme}
            platform={enPlatform}
            initialView={renderMineralsSheetView(homeState)}
          />
        </Animated.View>
      ) : null}
    </RNView>
  )
}
