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
- `src/sync/mobile-sync-host.ts` — host-owned Expo SQLite composition over the
  shared Khala Sync store core. It owns the native handle and installation
  identity plus a separate immutable device-local identity/local-authority
  store. The app is usable without OpenAuth; server-verified account linking
  is an optional, reversible upgrade and unlink/denial retains local rows. It
  composes production HTTP/WebSocket Sync only after session
  verification, subscribes the server-derived owner's personal scope, closes
  session-before-store on OTA reload/unmount, and never projects credentials,
  owner refs, or native handles into the Effect Native view program. The shared
  store migrates the supported unversioned cache in place and refuses a newer
  schema before mutation with typed update-or-reset guidance; sparse event
  batches replay from the durable cursor instead of skipping history.
- `src/conversation/mobile-conversation.ts` — public-safe adapter over the
  host-owned canonical conversation service. Startup selects confirmed Sync or
  the existing public-local path before Home mounts; exact stable refs must be
  confirmed before create/append appears complete. Once linked, the same host
  reads the bounded canonical agent timeline and submits shared exact-ref
  start, same-run follow-up, or interrupt commands. Home receives confirmed
  stream updates until terminal state appears or an explicit pending-
  reconciliation timeout is shown.
- `src/auth/native-session-vault.ts` — one versioned Expo SecureStore record
  for the native OpenAgents access/refresh tokens and server-derived owner ref.
  Recovery exposes only signed-out or credential-present-unverified state;
  malformed/retired records are purged and no credential enters view state.
- `src/auth/native-session-recovery.ts` — host-only validation against the
  existing native session endpoint. It passes the refresh token only on that
  route, persists OpenAuth rotation, purges denial/owner mismatch, and returns
  only signed-out/verified/denied/unavailable state.
- `src/auth/native-session-pkce.ts` — host-only GitHub authorization-code +
  S256 entry and dual-revocation exit. It uses the exact public client and
  canonical `openagents://auth`, verifies the server owner before saving, and
  never exposes credential material to the Effect Native program.
- Styling is **typed style objects on the shared token vocabulary** — no
  NativeWind, no Tailwind class strings (see
  `docs/effect-native/2026-07-08-styling-tailwind-stylex-effect-native.md`).

New component needs go upstream through the effect-native GAPS/demand register
(EN-2 #8572) — never app-local one-off primitives.

## Run it

From the repo root (installs the vendored `@effect-native/*` workspace
packages):

```sh
pnpm install
```

### Development (custom development build / simulator)

```sh
cd apps/openagents-mobile
pnpm run dev          # expo start — press i for iOS simulator or a for Android
```

The app tree is Effect Native. On iOS 26+, `@effect-native/render-rn` lowers
glass toolbar/composer nodes internally through `@expo/ui`; Android, older iOS,
tests, and missing-module hosts use the renderer-owned React Native material
fallback. Verify native glass, keyboard, and safe-area behavior through a
development or TestFlight build rather than Expo Go.

### Tests

```sh
cd apps/openagents-mobile
pnpm test             # or from the root: pnpm run test:openagents-mobile
```

The tests drive the REAL `@effect-native/render-rn` renderer against a
string-typed host shim (the same technique as the renderer's own parity
tests), so the view program, theme resolution, and the typed intent
loop (tap -> intent -> handler -> state -> re-render) are proven without a
simulator.

### Device / native builds (local only — never `eas build`)

```sh
cd apps/openagents-mobile
pnpm run prebuild:ios       # expo prebuild --platform ios
open ios/OpenAgents.xcworkspace
# build/run from Xcode (Apple Team HQWSG26L43), or:
#   xcodebuild -workspace ios/OpenAgents.xcworkspace -scheme OpenAgents ...
pnpm run prebuild:android   # expo prebuild --platform android
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
pnpm run publish:ota        # = apps/oa-updates/scripts/publish-ota.sh
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

## Effect Native glass lowering

The application-local `modules/openagents-liquid-glass` island is deleted.
`home-core.ts` and `khala-core.ts` own one serializable Effect Native tree:
typed `Toolbar`, `IconButton`, transcript, drawer, and `Composer` nodes. The
React Native screen owns only safe-area, keyboard avoidance, and one
`EffectNativeHost` mount. `@expo/ui` remains a native installation vehicle but
is loaded only inside `@effect-native/render-rn`; app source cannot import it.
The component-sharing oracle fails on a restored app-local module, direct
native-UI import, RN `Pressable`/`TextInput` application controls, or a second
composer.

## What exists today

The Home screen is persona-neutral: after native-session recovery it mounts one
conversation authority. Live personal Sync reconstructs confirmed canonical
threads/messages in the existing Effect Native transcript, drawer, and
renderer-lowered glass composer. Signed-out/not-live startup retains the public Khala
orchestration path. These catalogs never merge; explicit auth transitions
dispose and remount Home. Sync mutations use stable mobile refs, render drafts
as pending, and replace them only after exact-ref confirmation. Selected coding
sessions restore their canonical private draft and turn the composer's plus
control into a native multi-file/image picker. Each selection is bounded,
SHA-256 addressed, copied into the durable app document sandbox, and represented
in the draft only by ready metadata plus an `attachment.native-local.sha256.*`
ref; raw picker URIs never enter view state or Sync. A private
Expo SQLite cache now supplies restart-stable local Khala Sync storage and an
honest `Local Sync ready` state. Native session credentials now have a
device-only SecureStore vault, but a recovered record remains visibly
unverified until the server accepts it. Startup now validates that record and
persists access/refresh rotation; denial or owner mismatch purges it. A verified
session now starts authenticated personal-scope Sync in the Expo host. Session
readiness remains distinct from the live-only confirmed conversation service.
The surface enters through a typed GitHub sign-in intent and exits only after the server proves
both access and refresh revocation. The app does not manufacture fleet,
account, receipt, or cross-device authority. Durable runtime commands stay
pending until confirmed projection.

The host owns a bounded canonical conversation service once personal
Sync is live. It lists only confirmed `chat_thread` / `chat_message` projections
with public-safe refs and server versions, opens exact thread scopes, and sends
the canonical create/append mutations. The cross-native e2e proves a Desktop
thread and first message can be continued by mobile and reconstructed by both
stores after restart. Home consumes that confirmed service without receiving an
owner ref, credential, store/session object, transport, or raw row.
Provider-neutral assistant/runtime events enter Home only as bounded canonical
timeline items; raw provider payloads never enter native state. A deployed-
account/physical-device receipt remains the explicit #8676 close gate.

The shared lifecycle fence discards delayed responses from a superseded mobile
subscription, refuses provider events whose sequence/state no longer matches
the durable turn, and clears hosted scopes plus queued mutations on proven
unlink/revocation. Real Expo SQLite and Desktop SQLite tests close/reopen the
same in-flight timeline and converge on one server-projected interrupted
terminal without duplicate assistant output. This deterministic result does
not replace the still-pending physical-iPhone network-gap receipt for
#8689/#8677.
