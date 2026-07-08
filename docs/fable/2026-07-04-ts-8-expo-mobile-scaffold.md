# TS-8 Expo Mobile Scaffold

**STATUS (2026-07-08): SUPERSEDED by `docs/fable/MASTER_ROADMAP.md`
§EN (rev 6) — the Effect Native full-conversion mandate.** Kept as
the historical record of the earlier decision; do not implement
from this document.


Date: 2026-07-04, evidence passes 2026-07-05 (x2)
Issue: #8350
Status (2026-07-05, round 2): both platforms build clean, LOCAL, from zero on
this Mac — iOS `** BUILD SUCCEEDED **`, Android `BUILD SUCCESSFUL` (real
`app-debug.apk` produced) — plus two real TestFlight uploads (both `VALID`,
independently reconfirmed via the App Store Connect API). The native STT
push-to-talk button and the Apple FM readiness card are wired into the routed
chat composer and settings screen. Only two items remain, both owner-gated:
one signed OTA round-trip proof (blocked on interactive `gcloud` re-auth), and
real-device parity for the native STT/Apple FM captures (their Swift/Kotlin
bodies still intentionally fail closed pending a physical-device pass). The
delegation-prompt validator has no live caller in either the mobile or Swift
reference app today — verified as parity with the equally-unwired Swift
`validateCodingPrompt`, not a mobile-app regression.

## Landed

- Added `clients/khala-mobile`, an Expo SDK 57 React Native app with Expo
  Router routes for Chat, Fleet, and Settings.
- Wired NativeWind to `openAgentsNativeWindTokens` from `@openagentsinc/ui`.
  NativeWind 4 currently rejects Tailwind 4 at Metro build time and loads the
  Tailwind config through CommonJS/Jiti, so the mobile package intentionally
  pins a local Tailwind 3 compiler and reads the TS-9 parity-tested
  `@openagentsinc/ui/nativewind-tokens.cjs` bridge while preserving the shared
  token source.
- Added Khala Sync collection setup over TS-3:
  `chatThreadKhalaSyncCollectionOptions`,
  `fleetRunKhalaSyncCollectionOptions`, and preview read models decoded through
  the shared schemas.
- Added a Metro resolver shim for workspace TypeScript packages that use
  NodeNext `.js` import specifiers against `.ts` sources.
- Added `expo-db-sqlite-persistence` as the local Expo SQLite persistence
  adapter name and implementation over `expo-sqlite` for sync checkpoints and
  projection cache.
- Added secure key storage through `expo-secure-store`; no API key storage path
  touches SQLite or bundled config.
- Added delegation prompt validation that rejects local paths, Codex auth
  paths, bearer/API tokens, provider-secret env names, email addresses, and
  high-entropy strings.
- Added two Expo modules:
  `khala-push-to-talk-stt` and `khala-apple-foundation-models`. They autolink
  through `expo-modules-core` and fail closed until owner-device proof replaces
  shell readiness with streaming STT and Apple FM calls.
- Fixed both local iOS podspecs to compile Swift sources from the podspec root
  instead of a non-existent nested `ios/` directory. This lets the generated
  Expo modules provider import `KhalaPushToTalkStt` and
  `KhalaAppleFoundationModels` during a clean Xcode build.
- Aligned `build:ios:local` with the generated Expo native names:
  `ios/Khala.xcworkspace` and scheme `Khala`.
- Added `expo-system-ui` so Android prebuild applies the dark
  `userInterfaceStyle` contract without Expo's platform warning.
- Repointed `apps/oa-updates/scripts/publish-ota.sh` from the retired
  `AutopilotRemoteControl` path to `clients/khala-mobile`, with
  `OA_MOBILE_PLATFORM` and `OA_UPDATES_OWNER` knobs.

## Verification

```sh
bun run --cwd clients/khala-mobile test
bun run --cwd clients/khala-mobile typecheck
bun run --cwd clients/khala-mobile prebuild:ios
bun run --cwd clients/khala-mobile build:ios:local
bun run --cwd clients/khala-mobile prebuild:android
```

The tests pin:

- OpenAgents Updates URL: `https://updates.openagents.com/khala-mobile/manifest`.
- No `eas build`, `eas submit`, or `eas update` scripts.
- The OTA publish script references `clients/khala-mobile` and not the retired
  app path.
- NativeWind tokens match the shared StarCraft-blue token export.
- Khala Sync preview rows decode through the shared chat/fleet schemas.
- Secure key storage uses the Khala keychain service.
- Expo SQLite checkpoint persistence works through an injectable Expo SQLite
  module.

Native receipts produced on 2026-07-04:

- `bun run --cwd clients/khala-mobile prebuild:ios`: passed from no generated
  `ios/` directory.
- `bun run --cwd clients/khala-mobile build:ios:local`: passed with Xcode's
  generated `Khala` workspace/scheme after the local Expo module podspec fix.
- `bun run --cwd clients/khala-mobile prebuild:android`: passed from no
  generated `android/` directory after adding `expo-system-ui`.
- `bun run --cwd clients/khala-mobile build:android:local`: blocked before
  Gradle execution because this Mac shell reports `Unable to locate a Java
  Runtime`.

## Evidence pass — 2026-07-05

Re-verified every acceptance-criteria line item from a completely clean
`origin/main` worktree (fresh `bun install`, no reused `node_modules`/`ios`/
`android`), plus independent checks that do not rely on prior commit-message
claims.

**Confirmed real, with independent evidence:**

- `bun run --cwd clients/khala-mobile test`: 103/103 pass (grew from the 67
  pinned in the scaffolding commit as the Arcade UI harvest landed).
  `typecheck` is clean.
- Local iOS build reproduces from zero: `expo prebuild --platform ios` into a
  directory that did not previously exist, followed by `bun run
  build:ios:local`, ended in `** BUILD SUCCEEDED **`. Note: the note above about
  `Khala.xcworkspace`/scheme `Khala` is now stale — Expo names the generated
  Xcode project from the current `app.json` `name` ("Khala Code"), so a
  *clean* prebuild produces `ios/KhalaCode.xcworkspace` / scheme `KhalaCode`,
  which is what `package.json`'s `build:ios:local` script actually points at
  today. (The long-lived dev checkout at
  `/Users/christopherdavid/work/openagents` still has a pre-rename
  `ios/Khala.xcworkspace` on disk from before the app-metadata rename commit;
  that stale local artifact — not the repo — needs `rm -rf ios && bun run
  prebuild:ios` to pick up the current name. No repo change was needed; a
  same-diff edit/revert confirmed the checked-in script is already correct.)
- **Two real TestFlight uploads, independently confirmed via the App Store
  Connect API** (not just the commit-message claim): querying
  `GET /v1/apps?filter[bundleId]=com.openagents.khala.mobile` resolves to App
  Store Connect app "Khala Code" (id `6787620136`), and `GET
  /v1/apps/6787620136/builds` returns:
  - build 1, id `3bb487cf-73b6-470f-a2ee-867ee924426e`, uploaded
    2026-07-04T23:12:46-07:00, `processingState: VALID`
  - build 2, id `bb16234d-0cb0-4049-90c5-be9c65ac07e2`, uploaded
    2026-07-05T00:27:59-07:00, `processingState: VALID`
  This is a distinct app/bundle id from the SwiftUI `khala-ios` app
  (`com.openagents.khala`, separate TestFlight history, Delivery UUID
  `95107d96-…`, documented in
  `docs/mobile/2026-06-26-khala-testflight-release-runbook.md`). The
  "builds LOCAL + TestFlight via `xcrun altool`" criterion is met for iOS.
- `clients/khala-ios/Khala` stays buildable: source tree intact, actively
  touched as recently as 2026-07-04 (`#8354` cross-device dogfood preflight),
  and `xcodebuild -list -project Khala.xcodeproj` cleanly enumerates the
  `Khala`/`KhalaTests` targets and `Khala` scheme.
- `app.json`'s `updates.url` is `https://updates.openagents.com/khala-mobile/manifest`;
  `apps/oa-updates/scripts/publish-ota.sh` is genuinely repointed to
  `clients/khala-mobile` (`OA_MOBILE_APP_DIR`/`OA_UPDATES_OWNER=khala-mobile`),
  not the retired `AutopilotRemoteControl` path. A live `curl` against
  `https://updates.openagents.com/khala-mobile/manifest` (protocol v1 headers)
  returns a well-formed multipart/mixed Expo Updates response
  (`{"type":"noUpdateAvailable"}` for a probe runtime version) — the server
  already correctly serves this app's owner path.
- Keychain-held credentials are real: `KhalaAuthProvider` /
  `src/auth/khala-auth-store.ts` reads/writes only through
  `expo-secure-store`; there is no fallback to bundled config or SQLite for
  the sign-in token in a distributed build.

**Not actually met yet (more precise than earlier drafts of this doc):**

- **Native modules are stubs, and — more importantly — are wired to nothing
  live.** `modules/khala-push-to-talk-stt`'s `startRecognitionAsync` always
  throws `SpeechRuntimeUnavailableException`; `khala-apple-foundation-models`'s
  `getAvailabilityAsync` always returns `status: "blocked"`. Both are real,
  compiling Expo-modules-API ports (confirmed via the successful Xcode build
  above), so the porting mechanism is proven — but their only caller,
  `src/native/modules.ts`'s `readNativeReadiness`, is used exclusively from
  `src/legacy-screens/settings.tsx`, which no route file under `app/`
  references. They do not appear anywhere in the shipped, routed app today.
- **Delegation-prompt validation exists and is unit-tested, but is also
  wired to nothing live.** `src/security/delegation-prompt.ts` is a real,
  sensible validator (rejects local paths, Codex auth paths, bearer tokens,
  provider-secret env names, emails, high-entropy strings) — but it is not a
  direct port of an equivalent Swift function (no matching validator was found
  in `clients/khala-ios`; it looks like a fresh TS implementation of the
  general "Khala request safety guard" concept from the repo's delegation
  runbook). Its only caller is `src/legacy-screens/chat.tsx`, which — like the
  legacy settings screen above — is not referenced by any routed screen under
  `app/`. The live composer (`src/components/chat-composer.tsx`) and sync push
  path (`src/sync/use-khala-sync-push.ts`) do not call it.
- **Android Gradle build still blocked**: `prebuild:android` passes cleanly,
  but `build:android:local` cannot run on this Mac (`Unable to locate a Java
  Runtime`) — same blocker as before, still unresolved.
- **Signed OTA round-trip still unproven**, and currently blocked on an
  owner-only step: running `bun run publish:ota` requires `gcloud run deploy`
  against the shared `oa-updates` Cloud Run service, and the local `gcloud`
  session needs interactive re-authentication (`Reauthentication failed:
  cannot prompt during non-interactive execution`). This is a different,
  narrower blocker than "no dev build to prove pickup" — the manifest
  serving path is already live and correct; what's missing is (a) an
  interactive `gcloud auth login` from the owner, then (b) actually publishing
  a real export and confirming a running dev build/simulator instance picks
  it up.

## Evidence pass — 2026-07-05, round 2

Re-verified from a fresh, separately-isolated `origin/main` worktree (rebased
onto the latest `origin/main` mid-pass — the repo is very active — with a
fresh `bun install` and no reused `node_modules`/`ios`/`android`).

**Closed this pass:**

- **Android Gradle build now genuinely succeeds** (previously documented as
  blocked by "Unable to locate a Java Runtime"). That framing undersold the
  real gap: this Mac has Java (via `brew install openjdk`), but the *default*
  `openjdk` cask installs JDK 26, and Android's Gradle/AGP toolchain's
  `jlink`/`core-for-system-modules.jar` transform rejects it
  (`WARNING: A restricted method in java.lang.System has been called`,
  `jlink` exits 1). Pointing `JAVA_HOME` at the already-installed `openjdk@17`
  cask instead, and `ANDROID_SDK_ROOT`/`ANDROID_HOME` at the already-installed
  `android-commandlinetools` cask (Gradle then auto-installs the missing NDK
  27 + Build-Tools 36 + Platform 36 with license auto-accept on first run),
  got the build past every toolchain-level failure.
  - That surfaced a second, *real* bug in our own code:
    `khala-push-to-talk-stt`'s `startRecognitionAsync` always
    `throw`s, so Kotlin infers the `AsyncFunction<R, P0>` call's `R` as
    `Nothing` — illegal as a `reified` type parameter
    (`e: ... Cannot use 'Nothing' as reified type parameter`). This failed
    every clean Android build regardless of JDK/SDK setup, on both the old and
    new toolchain. Fixed by pinning the type explicitly:
    `AsyncFunction<Map<String, Any>, String?>("startRecognitionAsync") { ... }`
    (`Nothing` is a subtype of any type, so the always-throwing lambda still
    satisfies the pinned signature — only the reified-generic *inference* was
    the problem, not the runtime behavior).
  - With both fixes, `expo prebuild --platform android` (into a directory
    that did not previously exist) then
    `bun run build:android:local` (`./android/gradlew :app:assembleDebug`)
    → `BUILD SUCCESSFUL in 3m 58s`, producing a real
    `android/app/build/outputs/apk/debug/app-debug.apk`. `app.json`'s
    `android` block (`package: com.openagents.khala.mobile`, matching the iOS
    `bundleIdentifier`, plus `RECORD_AUDIO` permission) confirms this is one
    real shared codebase, not an iOS-only config with an inert Android
    placeholder.
  - Re-ran the iOS receipt in the same fresh worktree too:
    `expo prebuild --platform ios` (no prior `ios/`) then
    `bun run build:ios:local` → `** BUILD SUCCEEDED **`, confirming the prior
    pass's result reproduces independently of the specific checkout.
  - `bun run --cwd clients/khala-mobile test`: 133/133 pass; `typecheck`
    clean — both in the fresh worktree.
- **The "wired to nothing live" gap from the first 2026-07-05 pass is now
  stale** — a later same-day commit (`afb316491b`) wired the push-to-talk mic
  button into `src/components/chat-composer.tsx` (tap to start/stop
  dictation, still fails closed with a surfaced error until real native
  capture lands) and the Apple FM readiness probe into a new "On-device"
  settings card, then deleted the unreachable `src/legacy-screens/` entirely.
  Verified directly: `src/legacy-screens/` no longer exists;
  `chat-composer.tsx` imports and calls `usePushToTalk`;
  `app/(drawer)/settings.tsx` imports and renders `useOnDeviceReadiness`.
- **Re-confirmed the two TestFlight builds independently, a second time**,
  with a fresh JWT against the App Store Connect API
  (`GET /v1/builds?filter[app]=6787620136`): build 1
  (`3bb487cf-73b6-470f-a2ee-867ee924426e`, 2026-07-04T23:12:46-07:00) and
  build 2 (`bb16234d-0cb0-4049-90c5-be9c65ac07e2`, 2026-07-05T00:27:59-07:00),
  both still `processingState: VALID`. No new build has been uploaded since
  the first evidence pass.

**Refined, not closed — the delegation-prompt "gap" is parity, not a bug:**

- `src/security/delegation-prompt.ts`'s `validateDelegationPrompt` still has
  no live caller (confirmed: the only files referencing it are the module
  itself and its test). But direct comparison against the Swift reference app
  shows this is **parity, not a regression**: `KhalaClient.swift`'s
  `requestCodexTask`/`validateCodingPrompt` — the actual Swift analog (a typed
  `codex_agent_task` request with an explicit target Pylon ref, for the
  separate "Khala -> Pylon -> Codex" own-capacity coding-delegation runbook,
  *not* a filter on ordinary chat sends) — is *also* only called from
  `KhalaClientTests.swift`, never from `ChatView.swift`
  ("No voice, no delegation panel, no model picker — just chat"). Neither
  client ships an in-app coding-delegation panel, so wiring the mobile
  validator into the ordinary chat composer would not be porting an existing
  Swift behavior — it would be inventing a feature neither app has, and would
  risk false-positive-blocking normal chat messages that happen to contain a
  long token-like string or an email address. Left unwired, matching the
  Swift reference exactly. See the updated `README.md` Security section.

**Still open, both owner-gated:**

- **Signed OTA round-trip still unproven.** Re-confirmed the same blocker as
  the first pass: `gcloud run services describe ... --project openagentsgemini`
  fails with `Reauthentication failed: cannot prompt during non-interactive
  execution`, even though `gcloud auth list` shows `chris@openagents.com` as
  the active account — the cached token itself needs an interactive
  `gcloud auth login` refresh. No service-account key available locally with
  Cloud Run deploy scope for this project. Tracked in `NEEDS_OWNER.md`
  ("gcloud re-auth needed for Khala mobile OTA publish — 2026-07-05"); the
  manifest-serving path itself is independently reconfirmed live and correct.
- **Real-device STT/Apple FM parity still unproven.** The native module
  bodies intentionally fail closed (`SpeechRuntimeUnavailableException` /
  Apple FM `status: "blocked"`) until a physical or working simulator
  dev-client pass proves real capture — this needs a device session, not more
  source changes.

## Not Yet Closed

Issue #8350 stays open for exactly two owner-gated items:

- One signed OpenAgents Updates OTA round-trip against a dev build (blocked
  on interactive `gcloud` re-auth — see `NEEDS_OWNER.md`).
- Real-device parity proof for the native STT capture and Apple FM bridge
  (both are wired into the live UI and fail closed honestly today; they need
  a physical/working simulator device session, not more source changes).

Every other acceptance-criteria line item (Expo Router shell + NativeWind +
TS-3 chat/fleet read surfaces + expo-sqlite persistence; expo-modules ports of
both Swift native pieces with the SwiftUI app still buildable; local
`expo prebuild` + Xcode **and** Gradle builds green with real iOS + Android
artifacts; `xcrun altool` TestFlight uploads confirmed `VALID`; own-OTA
`updates.url`/manifest-serving wiring off Expo's CDN; keychain-only key
storage with no bundled secrets; delegation-prompt validation ported with
Swift-matching (unwired) parity) is independently verified done.
