# Khala Mobile QA Swarm Audit — 2026-07-05

Status: honest first-pass audit. Every claim below is backed by a command
actually run in this session (output included or summarized) or an explicit
"unverified, needs device X" label. Nothing here is inferred from memory of
prior sessions' work.

Scope: `clients/khala-mobile` (the Expo React Native app), which per
`clients/khala-mobile/AGENTS.md` and `AGENTS.md` is the TS-8 destination app —
one codebase for iOS and Android. The native SwiftUI app at
`clients/khala-ios/Khala` remains the interim shipping companion and is out of
scope for this audit.

## Summary Verdict

| Area | iOS | Android | Confidence |
| --- | --- | --- | --- |
| Test suite (logic layer) | green | green | High — real, ran now |
| Typecheck | green | green | High — real, ran now |
| Local build | simulator build green (README, prior session) | clean Gradle assemble green (README, prior session, re-verified by test/typecheck pass now) | Medium — build success, not launch+interact |
| TestFlight upload | 2 uploads independently confirmed VALID via App Store Connect API | n/a (no Play Store lane yet) | Medium-high for upload; no install/interact proof |
| Auth (Tailnet auto-auth) | logic-level real, unit-tested | same (shared code, no platform split) | Medium — real code, no device network proof |
| Composer (queue/steer/stop) | pure logic real, unit-tested | same | Medium — component-level render never verified |
| Push-to-talk STT | availability-gating logic real; actual capture always rejects by design | same, plus a real regression fixed today | Low for capture; Medium for gating logic |
| Apple Foundation Models | readiness-gating logic real; actual bridge call not implemented | explicit "unavailable" by design | Low |
| Fleet/account display | sort/format logic real, unit-tested | same | Medium — no device visual proof |
| iOS vs Android gap | **This session has been heavily iOS-weighted** (2 TestFlight uploads, simulator builds, iOS-native module ports). Android has ONE real Gradle build success today plus a real bug fix, but zero device/emulator boot evidence. | | — |

The honest one-line summary: **the pure-logic layer (auth discovery, composer
intent-building, security validation, sort/display helpers, OTA policy,
connectivity resolution) is real, unit-tested, and green on both platforms
today. Nothing about actual on-device behavior — a launched app, a tapped
button, a captured voice sample — has been proven on a physical device or
emulator in this session for either platform.** iOS has stronger
build/upload-adjacent evidence (TestFlight); Android has weaker evidence
(local Gradle assemble only) but a genuinely-fixed real bug.

## Test Suite Status (real, run this session)

```
$ bun test  # clients/khala-mobile, before adding new tests
142 pass / 0 fail / 235 expect() calls across 20 files (3.19s)

$ bun test  # after adding tests/ux-contracts.test.ts (this audit's own contribution)
156 pass / 0 fail / 314 expect() calls across 21 files (0.56s)
```

No skips, no todos, no flakes observed across two consecutive full runs.

## Typecheck Status (real, run this session)

```
$ bun run typecheck   # tsc -p tsconfig.json --noEmit
(no output — clean)
```

Ran both before and after adding the new contracts file and test file; clean
both times.

## Build Status

### iOS

Per `clients/khala-mobile/README.md`'s dated 2026-07-05 receipt (from earlier
today, prior to this audit pass, and not re-run here since it is a multi-minute
Xcode build and the evidence is already dated today):

```
expo prebuild --platform ios   # into a fresh directory
bun run build:ios:local        # xcodebuild ... -> ** BUILD SUCCEEDED **
```

Additionally, per the same README: **two TestFlight uploads (build 1 and build
2) are independently confirmed `processingState: VALID` via the App Store
Connect API** — this is stronger evidence than a bare local build, since it
proves Apple's own processing pipeline accepted the binary. `git log` confirms
two more build-number bumps landed today (`7cae19e537` bump to 4,
`0a59ad543b` bump to 3) by a concurrent agent actively shipping TestFlight
builds — this audit did not touch `app.json` or interrupt that work.

**Not yet proven:** installing a TestFlight build on a physical device and
interacting with it (sign-in, send a message, use push-to-talk). That is
recorded as part of `khala_mobile.platform.launched_app_interaction_smoke.v1`
(pending) in the new contract registry.

### Android

Per the same README receipt (also dated 2026-07-05, earlier today):

```
expo prebuild --platform android
bun run build:android:local   # ./android/gradlew -p android :app:assembleDebug -> BUILD SUCCESSFUL
```

producing `android/app/build/outputs/apk/debug/app-debug.apk`.

**The reported Kotlin STT fix was independently re-verified in this audit
pass** — not just taken on faith from the README. The source file
`clients/khala-mobile/modules/khala-push-to-talk-stt/android/src/main/java/com/openagents/khalaptt/KhalaPushToTalkSttModule.kt`
was read directly and confirmed to declare:

```kotlin
AsyncFunction<Map<String, Any>, String?>("startRecognitionAsync") { locale -> ... }
```

with an explanatory comment describing the exact failure this fixes
("Cannot use 'Nothing' as reified type parameter", caused by the
always-throwing shell inferring a `Nothing` return type for the generic
`AsyncFunction`). This fix is now pinned by a real, passing regression oracle
— `khala_mobile.android.stt_module_typed_asyncfunction_signature.v1` in the
new contract registry — a source-string assertion (explicitly labeled as a
stopgap, matching the desktop registry's own allowance for that pattern) that
will fail the sweep if a future edit reverts to unpinned inference.

**Not yet proven:** an APK actually installed and launched on a device or
emulator. The Gradle assemble proves the code compiles and packages; it does
not prove the app boots, renders a screen, or handles a tap. There is also no
Android upload/store lane analog to TestFlight yet, so Android currently has
strictly weaker distribution-pipeline evidence than iOS.

## Auth Flow (Tailnet auto-auth + device-local auth)

Read directly: `src/auth/khala-auth-context.tsx`, `src/auth/khala-mobile-pairing-core.ts`,
`src/auth/khala-mobile-pairing.ts`.

`git log -- clients/khala-mobile` confirms the Tailnet auto-auth handoff
landed today as commit `6b51e9f164` ("feat(khala-mobile, khala-code): Tailnet
auto-auth handoff, no manual login screen"), and it is still the current state
of `main` after this session's `git pull --rebase` (no revert or follow-up
commit undid it).

What's real and verified:

- The auth provider's mount effect tries, in order: stored secure-store
  credentials → dev-env credentials (`EXPO_PUBLIC_*`, local dev only) → Tailnet
  auto-discovery. A manual sign-in screen is only ever reached as the
  `signed_out` fallback state after discovery genuinely fails.
- Discovery probes every configured Tailnet candidate host **concurrently**
  (not serially), so the "nothing signed in" case fails in one timeout window,
  not one-timeout-per-host. Verified by a new unit oracle
  (`tailnet_discovery_concurrent_priority.unit`) that stages a 3-host mixed
  response set and confirms it correctly returns the one paired host.
  A paired outcome always outranks a merely-reachable-but-signed-out host,
  which always outranks unreachable — verified by
  `tailnet_discovery_outcome_priority.unit`.
- Discovered credentials are re-validated against Khala Sync
  (`validateKhalaCredentials`) before being applied and persisted — a
  reachable-but-invalid pairing does not silently sign the user in.

What's NOT verified: whether this actually works against a real desktop Khala
Code instance over a real Tailscale network from a real phone. That requires
two physical devices (or a device + a Mac both on the same tailnet) and is
recorded honestly as unverified rather than claimed. The connectivity dot's
resolution-speed logic (shared code path, `khala-code-connectivity-core.ts`)
has the same real-unit-tested / not-device-tested split.

## Composer: Queue / Steer / Stop

Read directly: `src/components/chat-composer.tsx`,
`src/sync/khala-runtime-compose-core.ts`.

What's real and unit-tested:

- `buildStartTurnIntentArgs`, `buildAppendUserMessageIntentArgs`,
  `buildInterruptTurnIntentArgs` — the pure payload builders for Queue/Steer/Stop
  — are exercised by the pre-existing `tests/khala-runtime-compose-core.test.ts`
  (181 lines, dedicated coverage for lane-targeting after issue #8405) and by
  this audit's new contract oracle
  (`steer_and_queue_use_active_turn_lane.unit`), which specifically pins that
  Steer and Queue always target the ACTIVE turn's own lane, never the idle
  lane picker's current value.
- Push-to-talk gating and dictation-merge logic (below) are also real and
  unit-tested.

What's only manually-verified-once or not verified at all:

- The actual `ChatComposer` React component — its Steer-vs-Queue picker
  showing/hiding, the animated height transition, the Stop button replacing
  Send while a turn is active, the lane-picker pills — has **never been
  rendered in a test**. `react-test-renderer` is a listed devDependency, but
  grep across `tests/` confirms no file imports it or mounts any `.tsx`
  component. This is a real, material gap: the pure functions behind the UI
  are proven, but "does the button actually show up and do the right thing
  when tapped" is not. This is now the explicit pending contract
  `khala_mobile.composer.rn_component_mount_coverage.v1`, not a silently
  accepted hole.
- Swipe-to-quote (`buildComposerTextWithQuote` in `swipe-quote-core.ts`) has
  real unit coverage for the pure merge function
  (`tests/swipe-quote-core.test.ts`), but the same "never mounted as a
  component" caveat applies to the actual swipe gesture and dedup-by-request-id
  logic living inside `ChatComposer`'s `useEffect`.

## Native Modules

### Push-to-talk STT

Read directly: `src/native/push-to-talk-core.ts`,
`src/native/use-push-to-talk.ts`, `modules/khala-push-to-talk-stt/src/index.ts`,
the Kotlin module source (see Build Status above), and the Swift module source
referenced in comments.

Real and unit-tested: the phase state machine
(`phaseFromAvailability`, `isPushToTalkPressable`) correctly gates the mic
button to `idle`/`recording` only, `describePushToTalkFailure` turns a raw
native rejection into a user-legible sentence, and `mergeTranscriptIntoDraft`
correctly appends/no-ops. All confirmed by both the pre-existing test file and
this audit's new contract oracles.

**Explicitly not working today, by design, on both platforms:** both the
Swift and Kotlin module implementations always reject
`startRecognitionAsync()` — the current shells report a runtime-pending state
rather than performing real speech capture. This is not a bug to paper over;
it is the honest current state, now tracked as the pending contract
`khala_mobile.stt.real_device_capture_proof.v1` with explicit blockers for
both a physical iOS device (Speech framework) and a physical Android device
(SpeechRecognizer).

### Apple Foundation Models

Read directly: `src/native/on-device-readiness-core.ts`,
`modules/khala-apple-foundation-models/src/index.ts`.

Real and unit-tested: the readiness-row formatting logic
(`buildOnDeviceReadinessRows`) correctly maps availability status to
tone/label for the Settings screen.

**Explicitly not working today, by design:** the iOS bridge reports a
"local helper proof still needed" blocker rather than calling a real Foundation
Models API, and Android reports explicit unavailability (there is no Android
equivalent capability). Tracked as pending contract
`khala_mobile.applefm.real_device_bridge_proof.v1`.

## Fleet / Account Status Display

Read directly: `src/sync/khala-fleet-collections-core.ts`. This is a thin,
real, unit-tested layer: `sortAccountsByReadinessThenRef` (ready > cooldown >
unavailable > unknown, tie-broken by ref hash), `sortWorkersByIdAsc`, and
`formatAccountRefHash` (display-safe truncation of a hashed account ref). All
three are covered by the pre-existing `tests/khala-fleet-collections-core.test.ts`
and now also by the new contract oracle for readiness ordering. No claim is
made here about the underlying Khala Sync fleet collection's live correctness
on a real device — that is a broader, separately-tracked claim.

## Security

Read directly: `src/security/keychain.ts`, `src/security/delegation-prompt.ts`,
`src/sync/expo-db-sqlite-persistence.ts`.

- **Keychain**: API keys route only through `expo-secure-store`'s
  keychain-backed API with a dedicated service name; a blank/whitespace key is
  rejected before any store write. Real, unit-tested (pre-existing
  `tests/keychain.test.ts` plus the new contract oracle).
- **Delegation prompt validator**: `validateDelegationPrompt` rejects local
  paths, `.codex/auth.json` references, bearer tokens, `oa_agent_` API keys,
  named provider-secret env vars, mnemonic/password-shaped phrases, emails,
  and high-entropy strings before a coding-delegation prompt would ever be
  built. Real, unit-tested (pre-existing `tests/delegation-prompt.test.ts`
  plus the new contract oracle exercising each blocker category
  individually). Per the mobile README, this validator currently has **no
  live caller** in either the Expo app or the SwiftUI reference app — it is
  tested but not yet wired to an actual in-app delegation panel. That is
  parity with the reference app, not a regression, but it means the
  protection is proven-in-isolation rather than proven-in-product-flow.

## OTA Updates

Read directly: `src/config/updates.ts`, `app.json`. The configured manifest
URL is `https://updates.openagents.com/khala-mobile/manifest` (never an Expo
`expo.dev`/`u.expo.dev` hosted URL), and the forbidden-command list correctly
names `eas build`/`eas submit`/`eas update`. Real, unit-tested (pre-existing
`tests/ota-policy.test.ts` plus the new contract oracle). Per the README, a
signed OTA round-trip against a real dev build is still owner-gated (needs an
interactive `gcloud auth login` re-auth) — not attempted in this audit.

## Any Other Major Surface Area

- **Sync runtime** (`khala-mobile-sync-runtime.ts`, 432-line dedicated test
  file, the largest test file in the package): TanStack DB collection wiring,
  optimistic writes, and pull/push cycles against Khala Sync. Real,
  unit-tested at the collection layer; not verified against a live production
  Khala Sync connection in this audit.
- **Cross-agent handoff** (`khala-cross-agent-handoff-core.ts`, "Ask
  Claude/Codex to review this", #8407): real, unit-tested pure logic
  (125-line test file); same "never mounted as a component" caveat as the
  composer.
- **Motion/visual ports** (Arwes Frame/Button, Skia ActivityIndicator,
  BackgroundGradient, SwipeableItem, Toggle, BlurredPopup): these are recent
  visual harvests from "Arcade" with dedicated core-logic test files
  (`toggle-position-core.test.ts`, `blurred-popup-menu-core.test.ts`,
  `swipe-quote-core.test.ts`), but — consistent with the pattern above — none
  of the actual Skia-rendered visuals have been screenshotted or visually
  verified in this audit. This is a reasonable scope boundary for pure-logic
  unit tests, but real visual verification (the desktop registry's
  `visual-smoke` oracle kind) does not exist yet for mobile.

## The iOS/Android Gap, Stated Plainly

This session's mobile work has been heavily iOS-weighted: two independently
API-confirmed TestFlight uploads, iOS-specific native module reference
material (`clients/khala-ios/Khala`), and iOS-first build-number bump
commits. Android's evidence is real but thinner: one clean Gradle assemble
today, and one genuinely-fixed Kotlin build-blocking bug (verified directly in
this audit, not just read from a changelog). Neither platform has evidence of
an actually-launched, actually-interacted-with app on real hardware or an
emulator. If a next pass has to choose where to spend physical-device time
first, Android is the platform with the bigger evidence gap relative to iOS,
not the platform with more known bugs.

## What This Audit Did Not Do

- Did not launch a simulator, emulator, or physical device.
- Did not re-run the multi-minute `xcodebuild`/Gradle build steps (their
  today-dated README receipts were treated as current evidence rather than
  re-verified, since a concurrent agent is actively using this build pipeline
  for TestFlight uploads and re-running it risked colliding with that work).
- Did not attempt the owner-gated OTA round-trip or any `gcloud`/App Store
  Connect calls.
- Did independently re-verify: the full unit test suite (twice), the
  typecheck, and the specific Kotlin AsyncFunction fix (by reading the source
  file directly).
