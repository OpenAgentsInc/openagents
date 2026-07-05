# Khala Mobile

Expo React Native companion for Khala. This is the TS-8 destination app: one
codebase for iOS and Android, NativeWind over the shared OpenAgents token
export, Khala Sync read models through `@openagentsinc/khala-sync-db-collection`,
and self-hosted OTA through OpenAgents Updates. NativeWind 4 currently requires
Tailwind 3 plus a CommonJS-readable Tailwind config at Metro build time, so this
mobile package pins a local Tailwind 3 compiler and reads the TS-9 parity-tested
`@openagentsinc/ui/nativewind-tokens.cjs` bridge. The React/Tailwind web
surfaces keep Tailwind 4. Mobile deliberately overrides only the shared
`bg` token from pure black to `#02060d`, matching the Arcade/StarCraft
direction's tinted void while leaving the web brand surfaces' black canvas
unchanged.

Metro also carries a local resolver shim for this monorepo's TypeScript
packages: shared packages use NodeNext `.js` import specifiers while source
files live as `.ts`, so Metro falls back from local `.js` requests to matching
`.ts`/`.tsx` files.

As of 2026-07-05, the app shell no longer uses Expo Router. The explicit
entrypoint is `index.tsx`, which registers `src/app.tsx`; signed-in app routes
live in typed React Navigation stacks/drawers under `src/navigators`, with
screens under `src/screens`. Keep new navigation work in that Ignite-style
navigator structure.

The app root also follows an Ignite-inspired provider spine:
`GestureHandlerRootView`, `SafeAreaProvider`, `KhalaThemeProvider`,
`StatusBar`, `KhalaAuthProvider`, then the signed-in navigator inside a
public-safe `KhalaErrorBoundary`. New ordinary screens and plain actions should
use `KhalaScreen`, `KhalaText`, and `KhalaButton`; keep `ArwesButton` and
Skia-heavy controls for intentional product moments.

The signed-in thread list is an intentional but restrained Arcade product
moment: it keeps the breathing `BackgroundGradient`, custom
`ActivityIndicator`, and `TouchableFeedback` primitives, but avoids per-row
Skia `Frame`/corner chrome. The first screen should scan like a calm mobile
inbox with Khala atmosphere, not like every row is competing to be the UI.

The native SwiftUI app at `clients/khala-ios/Khala` remains the interim
shipping companion and native-reference source until this app proves parity on
device.

## Local Development

```sh
bun install
bun run --cwd clients/khala-mobile dev
bun run --cwd clients/khala-mobile test
bun run --cwd clients/khala-mobile typecheck
bun run --cwd clients/khala-mobile architecture:check
```

## Architecture Guardrails

Khala Mobile has a package-local Dependency Cruiser check inspired by Ignite:

```sh
bun run --cwd clients/khala-mobile architecture:check
```

The config lives at `clients/khala-mobile/.dependency-cruiser.cjs`. It checks
for production imports from tests, unknown/missing package dependencies,
production imports from devDependencies, domain modules importing route owners,
and native module runtime imports outside `src/native/modules.ts`. Circular
dependencies are an error: the React Navigation migration settled with a clean
graph (zero cycles across `index.tsx`, `src`, and `tests`), so `no-circular`
was tightened from warn to error in #8454 and any new cycle now fails
`architecture:check`.

Documented exceptions are intentionally narrow: Bun's built-in test/plugin
module is allowed for test support, and type-only native package imports are
allowed in pure native helpers while runtime native access remains centralized.

Local Ignite-style scaffolds live under
`clients/khala-mobile/templates`. They are plain `.ejs` starting points for
screens, components, navigators, and UX-contract oracle tests; the app does not
depend on the Ignite CLI at runtime.

## Local Maestro Smoke Flows

Maestro flows are local-only device/simulator checks. They do not use EAS,
GitHub Actions, hosted CI, or committed credentials.

```sh
MAESTRO_APP_ID=com.openagents.khala.mobile \
  maestro test clients/khala-mobile/.maestro/flows/LaunchFallback.yaml
```

The signed-in thread smoke needs a public-safe seeded account and thread. Use
environment variables or a Metro bundle started with
`EXPO_PUBLIC_KHALA_SYNC_DEMO_OWNER_USER_ID` and
`EXPO_PUBLIC_KHALA_SYNC_DEMO_TOKEN`; never commit real values.

```sh
MAESTRO_APP_ID=com.openagents.khala.mobile \
KHALA_MAESTRO_OWNER_USER_ID="<owner-user-id>" \
KHALA_MAESTRO_TOKEN="<token-from-local-env>" \
KHALA_MAESTRO_THREAD_TITLE="<public-safe-thread-title>" \
  maestro test clients/khala-mobile/.maestro/flows/SignedInThreadSmoke.yaml
```

`LaunchFallback.yaml` proves launch plus a real no-credential fallback
interaction. `SignedInThreadSmoke.yaml` is the path toward closing
`khala_mobile.platform.launched_app_interaction_smoke.v1`, but that contract
stays pending until a dated device/emulator run receipt is recorded.

## Local Builds

Build and submit locally only.

```sh
bun run --cwd clients/khala-mobile prebuild:ios
bun run --cwd clients/khala-mobile build:ios:local

bun run --cwd clients/khala-mobile prebuild:android
bun run --cwd clients/khala-mobile build:android:local
```

For TestFlight, use the native Apple lane after the Xcode archive/export step:

```sh
xcrun altool --upload-app --type ios --file path/to/Khala.ipa \
  --apiKey "$APP_STORE_CONNECT_API_KEY_ID" \
  --apiIssuer "$APP_STORE_CONNECT_ISSUER_ID"
```

Team: `HQWSG26L43`.

2026-07-05 receipt: both platforms build clean from zero on this Mac. iOS —
`expo prebuild --platform ios` into a directory that did not previously exist,
then `bun run build:ios:local` → `** BUILD SUCCEEDED **`. Android — the
earlier "Java is not installed" blocker was a `JAVA_HOME`/toolchain gap, not a
missing capability: with `JAVA_HOME` pointed at a JDK 17 install (Android's
Gradle/AGP/jlink toolchain rejects a bare JDK 26) and `ANDROID_SDK_ROOT` set to
an installed `android-commandlinetools` SDK (Gradle auto-installs the matching
NDK/build-tools/platform on first run), `expo prebuild --platform android`
then `bun run build:android:local` → `BUILD SUCCESSFUL`, producing
`android/app/build/outputs/apk/debug/app-debug.apk`. Getting there also
surfaced and fixed a real bug: `khala-push-to-talk-stt`'s
`startRecognitionAsync` always threw, so Kotlin inferred its `AsyncFunction`
return type as `Nothing` — illegal as a `reified` type parameter
(`Cannot use 'Nothing' as reified type parameter`), which failed every clean
Android build regardless of Java/SDK setup. Fixed by pinning the type
explicitly (`AsyncFunction<Map<String, Any>, String?>(...)`).

2026-07-05, later same day — first real simulator launch, root cause found
for the recurring "No script URL provided" red screen that blocked every
prior TS-8 device-parity attempt (#8393/#8395/#8398/#8399, and earlier passes
on #8350): **this app never actually depends on `expo-dev-client`** (not in
`package.json`), so the built binary's `AppDelegate.bundleURL()` always uses
plain React Native's default `RCTBundleURLProvider` (Metro on `localhost:8081`
in `DEBUG`), not the `expo-dev-client` deep-link handshake
(`khala://expo-development-client/?url=...`). The `dev` script previously ran
`expo start --dev-client`, which prints that deep-link/QR instruction and
misled every prior pass into deep-linking a launcher that isn't wired
natively — the app has no code path that reads that URL. Fixed by dropping
the flag (`"dev": "expo start"`); the deep link is not needed and should not
be used. **Verified working end to end** on a booted `iPhone 17 Pro`
simulator (iOS 26.5) from a completely fresh worktree/build: `expo prebuild
--platform ios` → `bun run build:ios:local` → `** BUILD SUCCEEDED **` →
`xcrun simctl install` → plain `expo start --port 8081` → `xcrun simctl
launch` → Metro bundled cleanly at the then-current Expo Router entry
(`iOS Bundled 3742ms ...expo-router/entry.js (2454 modules)`) → the app rendered its real Tailnet auto-auth fallback
screen ("No signed-in Mac found on your Tailnet" / Retry / Sign in manually),
matching the documented auth-provider behavior exactly. This is the first
session to get a `khala-mobile` build past install+launch+render on any
simulator or device.

Real STT/Apple FM capture parity remains unproven, but not because of
simulator/device access — it is unproven because **neither platform's native
module actually attempts capture yet**: `KhalaPushToTalkSttModule.swift`'s
`startRecognitionAsync` unconditionally `throw`s
`SpeechRuntimeUnavailableException` and the Kotlin module unconditionally
throws `CodedException("android_stt_runtime_pending")`; `getAvailabilityAsync`
on `KhalaAppleFoundationModelsModule.swift` unconditionally returns
`"blocked"` regardless of device state. This is deliberate current scope, not
a bug — but it means no physical device, simulator, or emulator session (this
one included) can produce a positive capture proof until real
`SFSpeechRecognizer`/`SpeechRecognizer` and Foundation Models bridging code
lands; the reference SwiftUI app (`clients/khala-ios/Khala`) also has no
`import Speech` or STT implementation to be "at parity" with today. The
composer's mic button and the Settings "On-device" section do render and
correctly surface each module's fail-closed error/status when the routed UI
reaches them (verified by unit test + direct source read); reaching that
specific screen live in this pass would have required real Khala Sync sign-in
credentials (`EXPO_PUBLIC_KHALA_SYNC_DEMO_OWNER_USER_ID`/`_TOKEN`) or UI-tap
automation (no `idb`/Maestro/Appium available in this environment), neither of
which this pass exercised.

## OTA Updates

`app.json` embeds:

```text
https://updates.openagents.com/khala-mobile/manifest
```

Publish through the OpenAgents Updates server:

```sh
bash apps/oa-updates/scripts/publish-ota.sh
OA_MOBILE_PLATFORM=android bash apps/oa-updates/scripts/publish-ota.sh
```

The script computes the Expo runtime fingerprint, runs `expo export`, seeds
`apps/oa-updates`, signs the manifest when the local signing key is present, and
deploys the OpenAgents server. It does not call Expo hosted update commands.

## Native Modules

- `modules/khala-push-to-talk-stt` ports the Swift Speech push-to-talk seam
  through `expo-modules-core`. The current shell reports permission/runtime
  readiness and fail-closes actual streaming capture until owner-device proof.
- `modules/khala-apple-foundation-models` ports the Apple FM bridge readiness
  seam. iOS reports the local helper proof blocker; Android reports explicit
  unavailability.

## Security

- `src/config/public-config.ts` is the named boundary for Expo-bundled public
  metadata and endpoints (`app.json` / `extra.khala`). Bundled config is public:
  never place API keys, bearer tokens, private endpoints, customer data, or
  other credential material there.
- `src/network/mobile-problem.ts` is the public-safe HTTP/sync problem
  classifier. Mobile request boundaries should map status, transport, timeout,
  and malformed-response failures through this seam instead of surfacing raw
  response bodies, provider payloads, tokens, or chat content.
- `src/preferences/nonsecret-preferences.ts` is the only local preference
  wrapper for safe UI/onboarding state. It uses a dedicated Expo SQLite
  database and a fixed key set; do not use it for API keys, bearer tokens,
  session material, chat bodies, prompts, or Khala Sync projection rows.
- API keys are stored through `expo-secure-store` with the Khala keychain
  service. SQLite stores durable Khala Sync cursors/checkpoints, confirmed
  projection rows, client identity, and pending mutation intents; it does not
  store bearer/API keys.
- `src/security/delegation-prompt.ts`'s `validateDelegationPrompt` is a real,
  unit-tested port of the Swift client's `validateCodingPrompt` (rejects local
  paths, Codex auth paths, bearer/API tokens, provider-secret env names,
  emails, and high-entropy strings) for the separate "Khala -> Pylon -> Codex"
  own-capacity coding-delegation runbook (a typed `codex_agent_task` request
  with an explicit target Pylon ref) — it is not a filter applied to ordinary
  chat messages. Parity note: the Swift reference app's equivalent
  (`KhalaClient.requestCodexTask` / `validateCodingPrompt`) is also only
  exercised from its test target today, not from any live `ChatView` call
  site. Neither app currently ships an in-app coding-delegation panel, so this
  validator has no live caller in either client yet; that is parity with the
  reference app, not a regression introduced by the port.
- Chat message bodies stay in authenticated Khala Sync scopes; public evidence
  may include only refs, counts, routes, and blocker IDs.

## Owner-Gated Proof Still Needed

Source-level scaffold, policy tests, local typecheck, and both local
prebuild+build receipts are agent-verifiable and green (see above and
`docs/fable/2026-07-04-ts-8-expo-mobile-scaffold.md`). Two TestFlight uploads
(build 1 and build 2, both `processingState: VALID`) are also independently
confirmed via the App Store Connect API. What's left needs owner/device
action:

- Produce one signed OTA round-trip receipt against a dev build — the
  manifest-serving path (`updates.openagents.com/khala-mobile/manifest`) is
  live and correct, but `publish-ota.sh`'s `gcloud run deploy` step needs an
  interactive `gcloud auth login` re-auth on this machine first (see
  `NEEDS_OWNER.md`). Re-checked 2026-07-05: still blocked — both
  `gcloud auth application-default print-access-token` and
  `gcloud run services list --account=<sa>` for the two locally-available
  service-account keys (`oa-vertex-inference@...`, `nexus-mainnet@...`) fail
  (reauth required / no Cloud Run permission on either SA), so there is
  currently no non-interactive path around this.
- Implement real native capture — `startRecognitionAsync` on both platforms
  and the Apple FM bridge's availability probe are still hardcoded to fail
  closed (see the 2026-07-05 simulator-launch note above); this needs actual
  `SFSpeechRecognizer`/Android `SpeechRecognizer` and Foundation Models
  integration code, which no device or simulator session can substitute for.
  Only after that lands does "device-proven STT stream and Apple FM bridge
  calls" become a meaningful proof target; retire the SwiftUI app only after
  parity.
