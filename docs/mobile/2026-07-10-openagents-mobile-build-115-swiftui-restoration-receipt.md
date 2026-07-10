# OpenAgents mobile — v0.5.2 build 115: SwiftUI Liquid Glass restoration

- Date: 2026-07-10
- Source commit: `ee78dc1a2e921cc42de365e48c5b1ec5976e51c1`
- App identity: `OpenAgents` / `com.openagents.app` / Team `HQWSG26L43`
- App Store Connect build: `115`
- App Store Connect build ID: `3892d5e6-00d7-4175-a0a2-b3e48dc4ced2`
- Distribution state: `VALID`

## Why a new native build is required

Build 114 removed `openagents-liquid-glass`, the application-local native
module which owns the iOS SwiftUI Liquid Glass controls. Without it, the
JavaScript fallback became the actual visible chrome: opaque outlined buttons
instead of the prior native SwiftUI material. The regression was discovered by
an owner device screenshot, not inferred from a structural test.

An over-the-air JavaScript update cannot install a missing native module into
build 114. Build 115 embeds the module again, so a new TestFlight build is the
required correction.

## Restored boundary

- `OpenAgentsLiquidGlassView.swift` owns the iOS 26 `.glassEffect` rendering
  and its older-iOS material fallback.
- `GlassComposer`, `GlassIconButton`, and `GlassPill` are native SwiftUI views
  invoked from the React Native shell for the visible mobile chrome.
- The conversation state, typed intents, persistence, Sarah client, and Khala
  client remain shared JavaScript/Effect state. Only the presentation lowering
  that must be SwiftUI is native.

## Verification

- `bun run --cwd apps/openagents-mobile typecheck` — passed.
- `bun run --cwd apps/openagents-mobile test` — passed: 40 tests / 220
  expectations.
- `expo prebuild --platform ios --clean` produced `CFBundleVersion=115`.
- The Release archive and manual App Store export completed successfully.
- The IPA was uploaded through App Store Connect with the configured API key.
- App Store Connect confirms build 115 as `VALID`.

## Remaining owner proof

App Store processing is distribution evidence, not visual acceptance. On an
iOS device running build 115, verify that the top mode pill, icon buttons, and
composer are native Liquid Glass and that Khala remains selectable from the
same native mode picker.
