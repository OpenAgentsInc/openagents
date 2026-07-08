# Khala Mobile — build 21 (iOS TestFlight) + on-device send→reply proof (2026-07-07)

Fresh builds cut from `origin/main` (`82e436cb7b`, the deployed chat-fix head)
after the mobile chat send→reply loop was fixed and deployed server-side
(model now `gemini-3.5-flash`). The mobile app code was not the bug; these
builds re-cut current `main` and prove the loop works on a device.

## 1. iOS TestFlight build 21 — UPLOADED + VALID

- App: `Khala Code` (`com.openagents.khala.mobile`, App Store Connect app id
  `6787620136`, Team `HQWSG26L43`).
- `buildNumber` 20 → 21 in `clients/khala-mobile/app.json`.
- Path: `expo prebuild --platform ios` → `pod install` →
  `xcodebuild -workspace ios/KhalaCode.xcworkspace -scheme KhalaCode
  -configuration Release -destination 'generic/platform=iOS' archive`
  (`-allowProvisioningUpdates` + ASC API key) → fastlane App Store
  provisioning profile including the current Apple Distribution cert →
  `xcodebuild -exportArchive` with **manual** signing → `xcrun altool
  --upload-app`. This mirrors `docs/mobile/2026-06-26-khala-testflight-release-runbook.md`
  (the native-Khala runbook), adapted for the Expo app's `KhalaCode`
  workspace/scheme and `com.openagents.khala.mobile` bundle.
- Result: `** EXPORT SUCCEEDED **`, then `UPLOAD SUCCEEDED with no errors`,
  Delivery UUID `697cb207-ad77-4bfc-834f-6658782a0b4a`.
- App Store Connect: **build 21 VALID** (uploaded 2026-07-07 21:22 PT).
- Archived for a generic iOS device only; the booted simulators were not
  touched by this archive.

## 2. Clean non-baked Android release APK

- `expo prebuild --platform android` + `./android/gradlew :app:assembleRelease`
  with **no** `.env.local` (no baked credentials).
- Verified the release APK's JS bundle contains **no** seeded owner id and
  **no** `oa_agent_` token literal.
- On `emulator-5554` it launches to the sign-in screen
  (`KHALA CODE` + `Log in with GitHub`) — it does **not** auto-sign-in.
- Staged (uncommitted, gitignored location) at `~/work/khala-mobile-testing.apk`.

## 3. On-device send → assistant reply E2E (the key proof)

- A separate **baked** Android release build (`EXPO_PUBLIC_KHALA_SYNC_DEMO_*`
  = seeded public-safe agent creds; `.env.local` written only for that build
  and removed on exit) auto-signs-in and opens the seeded thread.
- Installed on `emulator-5554`; seeded thread turn state reset first.
- Sent on-device (default `hosted_khala` lane): **"What is the capital of
  Japan? Answer with one word."** (a prompt whose answer was confirmed absent
  from thread history beforehand).
- Server answered ~49s later. API cross-check via `/api/sync/bootstrap`
  (thread scope):
  - turn `turn.mrbkt4lqb3ba1f177d`, lane `hosted_khala`, model
    `gemini-3.5-flash`, **status `completed`**.
  - `text.delta` → **`"Tokyo"`**, `text.completed`, `turn.finished`
    (`finishReason: stop`).
- The reply **"Tokyo"** renders in the thread on device (captured to a
  gitignored screenshot path). This is the real device demonstration that a
  sent message gets an assistant reply.
- Observation: the assistant reply surfaces on the next thread bootstrap
  (relaunch/open) rather than instantly while idle on the thread screen —
  a live-refresh latency nuance, not a broken loop; the loop itself is green.

## Cleanup

- All `.env.local` files were removed after the baked builds (verified). No
  shippable artifact carries the seeded token; the committed tree has no
  secrets, keystores, or provisioning material.
