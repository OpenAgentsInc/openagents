# TS-8 Expo Mobile Scaffold

Date: 2026-07-04, evidence pass 2026-07-05
Issue: #8350
Status: iOS local build + two real TestFlight uploads (both `VALID`,
independently confirmed via the App Store Connect API) are proven. Native
STT/Apple FM modules and delegation-prompt validation exist as tested code but
are not wired into any routed screen. Android Gradle build and the signed OTA
round-trip remain blocked (Java runtime; interactive `gcloud` reauth).

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

## Not Yet Closed

Issue #8350 should remain open until:

- Local Gradle build receipt on a machine with a Java runtime and Android SDK.
- One signed OpenAgents Updates OTA round-trip against a dev build (currently
  blocked on interactive `gcloud` re-auth, see above).
- The native STT/Apple FM modules and the delegation-prompt validator are
  actually wired into the live, routed screens (today they are tested but
  unreachable dead code), then device-proven for real parity with the
  SwiftUI reference app.
