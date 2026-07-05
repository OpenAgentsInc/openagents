# Khala Mobile

Expo React Native companion for Khala. This is the TS-8 destination app: one
codebase for iOS and Android, NativeWind over the shared OpenAgents token
export, Khala Sync read models through `@openagentsinc/khala-sync-db-collection`,
and self-hosted OTA through OpenAgents Updates. NativeWind 4 currently requires
Tailwind 3 plus a CommonJS-readable Tailwind config at Metro build time, so this
mobile package pins a local Tailwind 3 compiler and reads the TS-9 parity-tested
`@openagentsinc/ui/nativewind-tokens.cjs` bridge. The React/Tailwind web
surfaces keep Tailwind 4.

Metro also carries a local resolver shim for this monorepo's TypeScript
packages: shared packages use NodeNext `.js` import specifiers while source
files live as `.ts`, so Metro falls back from local `.js` requests to matching
`.ts`/`.tsx` files.

The native SwiftUI app at `clients/khala-ios/Khala` remains the interim
shipping companion and native-reference source until this app proves parity on
device.

## Local Development

```sh
bun install
bun run --cwd clients/khala-mobile dev
bun run --cwd clients/khala-mobile test
bun run --cwd clients/khala-mobile typecheck
```

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
  `NEEDS_OWNER.md`).
- Replace the native-module shells with the device-proven STT stream and Apple
  FM bridge calls, then retire the SwiftUI app only after parity.
