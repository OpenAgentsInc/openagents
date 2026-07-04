# TS-8 Expo Mobile Scaffold

Date: 2026-07-04
Issue: #8350
Status: source scaffold landed; device/signing proof still owner-gated.

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
- Repointed `apps/oa-updates/scripts/publish-ota.sh` from the retired
  `AutopilotRemoteControl` path to `clients/khala-mobile`, with
  `OA_MOBILE_PLATFORM` and `OA_UPDATES_OWNER` knobs.

## Verification

```sh
bun run --cwd clients/khala-mobile test
bun run --cwd clients/khala-mobile typecheck
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

## Not Yet Closed

Issue #8350 should remain open until the owner/device lane produces:

- `expo prebuild` plus local Xcode and Gradle build receipts.
- TestFlight upload via `xcrun altool` under Team `HQWSG26L43`.
- One signed OpenAgents Updates OTA round-trip against a dev build.
- Device-proven STT and Apple FM module parity with the SwiftUI reference app.
