# OpenAgents mobile

Greenfield **OpenAgents** iOS/Android app (#8597, epic #8566). One Expo React
Native codebase whose application/component/intent model is **Effect Native**;
React Native and Expo are host and renderer machinery only.

- Display name: `OpenAgents`
- iOS bundle identifier / Android application ID: `com.openagents.app`
- Icon: exact copy of the canonical Khala Code mobile icon (SHA-256 pinned by
  `tests/app-identity.test.ts`)

This is **not** a rename of `clients/khala-mobile`. That package is deprecated
and frozen as a parity/contract/native-module/migration reference. This app
imports nothing from it.

## Architecture

- `src/screens/home-core.ts` — the PURE view-program layer: state, typed
  intents (`defineIntent` + Effect Schema payloads), and a typed
  `@effect-native/core` component tree. No `react`, no `react-native`. This is
  the layer shared structurally with the web Effect Native consumers
  (`apps/openagents.com/apps/start`): one catalog, many hosts.
- `src/effect-native/effect-native-host.tsx` — the single mount point binding
  React + React Native into `@effect-native/render-rn`'s surface.
- `src/screens/home-screen.tsx` — thin RN shell: safe area + the Effect Native
  surface mounted with the shared Protoss-blue `khalaTheme` from
  `@effect-native/tokens`.
- Styling is **typed style objects on the shared token vocabulary** — no
  NativeWind, no Tailwind class strings (see
  `docs/effect-native/2026-07-08-styling-tailwind-stylex-effect-native.md`).

New component needs go upstream through the effect-native GAPS/demand register
(EN-2 #8572) — never app-local one-off primitives.

## Run it

From the repo root (installs the vendored `@effect-native/*` workspace
packages):

```sh
bun install
```

### Development (custom development build / simulator)

```sh
cd apps/openagents-mobile
bun run dev          # expo start — press i for iOS simulator or a for Android
```

The app carries an iOS SwiftUI Liquid Glass module, so verify the native
composer through a development or TestFlight build rather than Expo Go.

### Tests

```sh
cd apps/openagents-mobile
bun test             # or from the root: bun run test:openagents-mobile
```

The tests drive the REAL `@effect-native/render-rn` renderer against a
string-typed host shim (the same technique as the renderer's own parity
tests), so the view program, theme resolution, and the typed intent
loop (tap -> intent -> handler -> state -> re-render) are proven without a
simulator.

### Device / native builds (local only — never `eas build`)

```sh
cd apps/openagents-mobile
bun run prebuild:ios       # expo prebuild --platform ios
open ios/OpenAgents.xcworkspace
# build/run from Xcode (Apple Team HQWSG26L43), or:
#   xcodebuild -workspace ios/OpenAgents.xcworkspace -scheme OpenAgents ...
bun run prebuild:android   # expo prebuild --platform android
./android/gradlew -p android :app:assembleDebug
```

Per repo policy: builds are local (`expo prebuild` + Xcode/Gradle);
`eas build`/`eas submit`/`eas update` are never used.

### OTA updates (owned server, never EAS)

JS/OTA updates ship through the owned OpenAgents Updates server
(`apps/oa-updates`, `updates.openagents.com`) on this app's OWN channel
`openagents-production` (identity tests enforce the URL/channel and reject any
legacy khala/AutopilotRemoteControl feed):

```sh
cd apps/openagents-mobile
bun run publish:ota        # = apps/oa-updates/scripts/publish-ota.sh
# fingerprint -> expo export -> seed -> Cloud Run deploy; verify with the curl
# line the script prints (signed manifest for this build's runtime fingerprint)
```

Bump `BUNDLE_TAG` in `src/screens/home-core.ts` before publishing so the swap
is identifiable in diagnostics. The installed app polls for updates on a
**TEMPORARY aggressive 3-second cadence** (`TEMPORARY_OTA_POLL_INTERVAL_MS` in
`src/updates/ota-polling.ts` — dial down after owner testing): a published OTA
appears on device within ~3s, downloads, and reloads. Errors (offline etc.)
are soft and never crash the loop. Polling is a no-op in Expo Go/dev
(`Updates.isEnabled` false).

## SwiftUI Liquid Glass island

`modules/openagents-liquid-glass` owns the visible iOS menu, icon controls,
and the one working Khala text composer using SwiftUI `.glassEffect` on iOS
26 and a material fallback on earlier iOS. Its serializable state comes from
the Effect Native program; native events return only through typed intents.
Android and test hosts use a functional React Native fallback.

## What exists today

The Home screen is persona-neutral: a typed Khala conversation with one native
Liquid Glass composer, backed by the public orchestration endpoint. It does
not manufacture local fleet, account, Sync, receipt, or cross-device state.
Fleet supervision, authenticated identity, Khala Sync continuity, and durable
commands are the next #8597 priorities.
