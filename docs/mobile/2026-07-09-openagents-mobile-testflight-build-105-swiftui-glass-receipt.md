# OpenAgents mobile — v0.4.2 build 105 (iOS TestFlight): SwiftUI Liquid Glass on the Effect Native seam (2026-07-09)

Third TestFlight build of `apps/openagents-mobile` (#8597): a SwiftUI
"Liquid Glass" test section on the Home screen, rendered through the Effect
Native SwiftUI seam prescribed by
`docs/effect-native/2026-07-09-effect-native-swiftui-renderer-audit.md`.
Release path identical to the build-103/104 receipts. Never EAS.

## What the audit prescribed vs what shipped

- **Prescribed:** SwiftUI consumes the typed catalog; a true
  `render-swiftui` renderer is a later lane (EN-S0…S6). For a SwiftUI island
  inside an RN-hosted app TODAY, the audit's **interop case 2** applies: a
  `UIHostingController` boundary at a per-component seam, with events out as
  typed intents and state in as serializable props. Foreign views must NOT
  enter through untyped children/callbacks; new host kinds are reviewed
  catalog changes.
- **Reality check:** the vendored catalog v26 (and upstream `origin/main`,
  same pin `941acc87`) has a CLOSED `hostKinds` registry with **no SwiftUI
  kind**, and `render-rn` has no host-driver injection seam. Editing vendored
  packages is banned; gaps go upstream.
- **Shipped (the honest interim, marked):** `modules/openagents-liquid-glass`
  (local Expo module, iOS only) mounts SwiftUI via `UIHostingController`
  below the Effect Native surface in the Home shell. Its props
  (`glassIslandProps`) are a pure projection of the SAME Home program state
  (`SubscriptionRef`), and its button dispatches the typed `GlassPinged`
  intent through the SAME `IntentRegistry` the RN renderer's reporter uses:
  SwiftUI tap → typed intent → state → the Effect Native tree re-renders
  ("Glass intents received: N") AND the island's `tapCount` prop updates.
  No parallel component system, no untyped callbacks into app logic.
- **Demand loop:** register row **D-MB-02** in
  `docs/effect-native/DEMAND_REGISTER.md` → upstream
  [effect-native#70](https://github.com/OpenAgentsInc/effect-native/issues/70)
  (SwiftUI host kind / `render-swiftui` lane + `render-rn` host-driver seam).

## SDK/API reality — REAL Liquid Glass

Local toolchain: **Xcode 26.6 (17F113), iOS 26.5 SDK.** The island uses the
real iOS 26 Liquid Glass SwiftUI APIs behind availability guards:

- glass card: `.glassEffect(.regular.tint(accent.opacity(0.2)), in: .rect(cornerRadius: 16))`
- glass button: `.buttonStyle(.glass)`
- pre-iOS-26 runtime fallback: `.ultraThinMaterial` glass-morphism (the pod's
  deployment target is iOS 16.4, so the guards are load-bearing, not
  decorative).

## Proof split (stated honestly)

- **Unit-proven** (`tests/glass-seam.test.ts`, 20 tests green, typecheck
  clean): the labeled section in the typed tree, the pure prop projection,
  and the full typed round-trip `dispatchGlassTap()` → `GlassPinged` →
  state → re-render through the REAL `@effect-native/render-rn` renderer —
  exactly the call the native `onGlassTap` event triggers.
- **Device-proven only:** the SwiftUI rendering itself (UIHostingController,
  `.glassEffect`, `.buttonStyle(.glass)`, material fallback). That rung is
  the owner's TestFlight install of build 105. Expo Go/Android render an
  honest "island unavailable in this host" notice instead.

## Build 105 release receipt

- Commit on main: `e08f995430` (feature) — cut from a fresh worktree.
- `bunx expo prebuild --platform ios --clean` → pod `OpenAgentsLiquidGlass`
  autolinked (deployment target raised to iOS 16.4 by ExpoModulesCore).
- Archived identity: `0.4.2` / `105` / `com.openagents.app`;
  `OpenAgentsLiquidGlassModule.swift` + `OpenAgentsLiquidGlassView.swift`
  compiled clean against the iOS 26.5 SDK (`** ARCHIVE SUCCEEDED **`).
- `** EXPORT SUCCEEDED **` (manual signing, `com.openagents.app AppStore`
  profile, 14.4 MB IPA) → `xcrun altool --upload-app` →
  **`UPLOAD SUCCEEDED with no errors`**, Delivery UUID
  `8ed97619-5e76-4384-8a3f-4c21b46ea72b`.
- ASC `/v1/builds`: **build 105 `processingState=VALID`**
  (uploaded 2026-07-09T17:29:49-07:00).
- OTA reseeded for the NEW runtime fingerprint
  `53b9a159f285e0fece7ec9265b3a4a084bd57ffc` (native module changed the
  fingerprint; matches the archive's `EXUpdates.bundle/fingerprint`
  byte-for-byte). Live signed-manifest curl: `HTTP/2 200`, `keyid="main"`,
  `"branch":"openagents-production"`, correct runtime. The 3s OTA poll loop
  from build 104 remains active in 105.

## Owner test

1. Install **build 105 (0.4.2)** from TestFlight.
2. The Home screen shows the Effect Native surface (with the labeled
   "SwiftUI via Effect Native — test" lines) and, below it, the SwiftUI
   Liquid Glass card + glass button.
3. Tap the glass button: the EN tree's "Glass intents received" count and the
   island's "Typed intents from SwiftUI" line increment together — one typed
   intent, one state, two renderers.
