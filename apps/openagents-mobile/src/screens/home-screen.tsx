import { useEvent } from "expo"
import { useVideoPlayer, VideoView } from "expo-video"
import { useEffect, useMemo, useRef, useState } from "react"
import {
  Animated,
  Easing,
  Platform,
  Pressable,
  View as RNView,
} from "react-native"
import { SafeAreaView, useSafeAreaInsets } from "react-native-safe-area-context"

import { Effect, Stream } from "@effect-native/core/effect"
import { khalaTheme } from "@effect-native/tokens"

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
  renderChromeComposerView,
  renderChromeMenuButtonView,
  renderChromeNewChatView,
  renderChromePillView,
  renderContentView,
  renderDrawerView,
  renderMineralsSheetView,
  renderModeMenuView,
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
 *   2. Floating glass chrome — typed Effect Native trees (GL-1 #8647):
 *      IconButton/Button/Toolbar with `surface: "glass"`, mounted through
 *      `EffectNativeHost`. render-rn lowers them INTERNALLY through @expo/ui
 *      to real SwiftUI Liquid Glass on iOS 26+ and to the honest material
 *      approximation everywhere else (Android, Expo Go, iOS < 26) — the shell
 *      has no per-platform branches and NEVER imports @expo/ui. Every tap
 *      dispatches a typed intent through the program registry (the ONLY seam
 *      native events enter the program). The former app-local
 *      openagents-liquid-glass expo-module island is deleted (D-MB-02).
 *   3. Drawer overlay when `drawerOpen`: scrim (tap = DrawerToggled) + the EN
 *      drawer panel (a SECOND view projection of the SAME program state).
 */
const enPlatform = Platform.OS === "android" ? ("android" as const) : ("ios" as const)

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
          {/* Top row: sidebar toggle, mode pill, new chat — each a typed EN
              tree in its own host so the space between them stays
              touch-transparent (box-none), exactly like the island layout. */}
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
            <EffectNativeHost
              viewStream={program.chromeMenuButtonViewStream}
              report={program.report}
              theme={khalaTheme}
              platform={enPlatform}
              initialView={renderChromeMenuButtonView(homeState)}
            />
            <EffectNativeHost
              viewStream={program.chromePillViewStream}
              report={program.report}
              theme={khalaTheme}
              platform={enPlatform}
              initialView={renderChromePillView(homeState)}
            />
            <RNView style={{ flex: 1 }} pointerEvents="none" />
            {/* Search icon removed from the chrome (owner direction
                2026-07-09); search stays reachable in the drawer. */}
            <EffectNativeHost
              viewStream={program.chromeNewChatViewStream}
              report={program.report}
              theme={khalaTheme}
              platform={enPlatform}
              initialView={renderChromeNewChatView(homeState)}
            />
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
            <EffectNativeHost
              viewStream={program.chromeComposerViewStream}
              report={program.report}
              theme={khalaTheme}
              platform={enPlatform}
              initialView={renderChromeComposerView(homeState)}
            />
          </RNView>
          ) : null}
        </>
      ) : null}

      {/* 2b. The pill's surface-mode dropdown — a typed EN DropdownMenu
          (renders a fullscreen Modal while open; a zero-size marker View
          while closed). */}
      <RNView pointerEvents="box-none" style={{ position: "absolute", width: 0, height: 0 }}>
        <EffectNativeHost
          viewStream={program.modeMenuViewStream}
          report={program.report}
          theme={khalaTheme}
          platform={enPlatform}
          initialView={renderModeMenuView(homeState)}
        />
      </RNView>

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
