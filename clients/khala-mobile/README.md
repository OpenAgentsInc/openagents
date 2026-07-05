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

2026-07-04 receipt: iOS prebuild and local simulator build pass from a clean
generated `ios/` directory. Android prebuild also passes; local Gradle build on
this Mac is blocked before Gradle starts because Java is not installed.

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
- Delegation prompts pass `validateDelegationPrompt` before submission. The
  validator rejects local paths, Codex auth paths, bearer/API tokens,
  provider-secret env names, emails, and high-entropy strings.
- Chat message bodies stay in authenticated Khala Sync scopes; public evidence
  may include only refs, counts, routes, and blocker IDs.

## Owner-Gated Proof Still Needed

Source-level scaffold, policy tests, and local typecheck are agent-verifiable.
The full TS-8 acceptance still needs owner/device or Android toolchain work:

- Run the local Gradle build on a machine with Java and the Android SDK.
- Upload the first TestFlight artifact through `xcrun altool`.
- Produce one signed OTA round-trip receipt against a dev build.
- Replace the native-module shells with the device-proven STT stream and Apple
  FM bridge calls, then retire the SwiftUI app only after parity.
