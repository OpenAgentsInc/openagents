# OpenAgents mobile — v0.5.0 build 107: ChatGPT-style glass shell (GL-2 #8648, epic #8646)

The Home screen is now the owner's design target: floating Liquid Glass
chrome over a typed Effect Native program, with the left nav flyout drawer.
Per the hybrid decision
(docs/fable/2026-07-09-swiftui-expo-ui-and-the-effect-native-stdlib.md §6).

## Composition (state-in-props, intents-out — always)

- ONE Effect Native program (`src/screens/home-core.ts`): state
  (drawer/selection/recents/counters), 8 typed intents (DrawerToggled,
  NewChatPressed, RecentSelected, SearchPressed, SettingsPressed,
  ChatPillPressed, ComposerPressed, MicPressed), and TWO view projections —
  content surface + drawer panel — from the same `SubscriptionRef`.
- Floating glass chrome: SwiftUI Liquid Glass islands
  (`modules/openagents-liquid-glass`, grown from the 105/106 test island):
  circular `GlassIconButton` (sidebar / search / new-chat, SF symbols),
  `GlassPill` ("OpenAgents", sparkles), `GlassComposer` (plus / "Ask
  anything" / mic). iOS 26 `.glassEffect(.regular.interactive(), in:
  .circle/.capsule)`; `.ultraThinMaterial` fallback pre-26; honest RN
  fallbacks on Android/Expo Go. Every tap enters the program ONLY through
  `program.chrome.*` typed dispatchers.
- Drawer: EN COMPOSITION (decision doc §6 — deliberately not an island):
  Search / New chat rows, Recents section with selected-row highlight
  (surfaceRaised), Settings, `Bundle <tag>` footer. The shell overlays the
  drawer projection + scrim (v26 typed styles have no absolute positioning —
  z-order is host machinery; trees and intents are all EN).
- Composer text input is an interim tap target dispatching ComposerPressed
  (real bound TextField lands with the Sarah conversation surface). Mic
  dispatches MicPressed (voice-input host wiring later). Marked interim.

## Simulator pixel proof (upload gate; iPhone 17 Pro, Release, cliclick taps)

Committed under `docs/mobile/receipts/`:

1. `2026-07-09-build107-shell-closed.png` — closed shell: glass sidebar
   button, OpenAgents glass pill, glass search + new-chat circles, floating
   glass composer (plus / Ask anything / mic); counters all 0.
2. `2026-07-09-build107-drawer-open.png` — after ONE simulated tap on the
   SwiftUI sidebar button (typed DrawerToggled): flyout open with Search/New
   chat, Recents, "Welcome to OpenAgents" highlighted as selected, Settings,
   Bundle footer, scrim over content, chrome hidden.
3. `2026-07-09-build107-recent-selected.png` — after tapping the "Glass shell
   design" recent: drawer closed, content heading switched (RecentSelected →
   state → re-render).
4. `2026-07-09-build107-after-mic-tap.png` — after tapping the composer mic
   (SwiftUI event): status card shows `mic 1`.

Observation, recorded honestly: on the very FIRST launch after installing
over the still-running build 106, the counters showed spurious chrome events
(search 10, a NewChatPressed). It did NOT reproduce on two fresh relaunches
and a full tap session (all counters started 0 and only incremented on real
taps). Watched-for on TestFlight (which installs cleanly); if it recurs, it
gets its own lane.

## Tests

19 pass (bun) — new `home-shell-core.test.ts` drives the REAL render-rn
renderer through the full loop (chrome toggle -> drawer open, recent row tap
-> selection + close, chrome counters re-render); identity/OTA/catalog-sharing
oracles updated and green. Typecheck clean.

## GL-1 relationship (#8647)

The upstream catalog v27 glass set (IconButton, Toolbar, surface:"glass",
Sheet detents) is in flight in OpenAgentsInc/effect-native; when vendored,
the drawer rows and chrome upgrade from v26 Buttons/islands to catalog tags
(the shell shape and intents do not change). The SwiftUI islands remain the
sanctioned interim per the decision doc §7 and D-MB-02.

## Release receipt

- Version `0.5.0`, iOS build `107`, BUNDLE_TAG `2026-07-09.embedded-107`.
- Recorded in #8648/#8597 comments: archive/export/upload receipts + ASC
  `processingState`, and the OTA reseed for this build's runtime fingerprint.
