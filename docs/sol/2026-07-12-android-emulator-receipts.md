# Android emulator receipts — first CUT-program Android evidence

- Date: 2026-07-12
- Issues: [#8694](https://github.com/OpenAgentsInc/openagents/issues/8694)
  (CUT-14), cross-refs
  [#8692](https://github.com/OpenAgentsInc/openagents/issues/8692) (CUT-12),
  [#8707](https://github.com/OpenAgentsInc/openagents/issues/8707) (CUT-27)
- Authority: owner decision 2026-07-12 (recorded at `1eb9f1a95f` and in the
  [CUT-27 readiness audit](./2026-07-12-cut27-cutover-readiness-audit.md)):
  **nothing in the CUT program gates on physical Android; emulator evidence
  satisfies every Android leg.**
- Status: this document is the CUT program's FIRST Android evidence of any
  kind for the greenfield **OpenAgents** mobile app
  (`apps/openagents-mobile`, `com.openagents.app`). It proves the
  signed-out/fail-closed halves of the CUT-14 Android journeys on a real
  emulator install and names the authenticated halves as exact gaps. It does
  NOT close CUT-14 or CUT-12.

## Build under test

- Source: clean detached worktree at `origin/main` @ `1eb9f1a95f`
  ("docs(sol): owner decision — no gate on physical Android…"). No app source
  changes; the generated `android/` project is local-only and not committed.
- Build (repo policy: local prebuild + Gradle, never `eas build`):

  ```sh
  cd apps/openagents-mobile
  bunx expo prebuild --platform android --no-install
  cd android
  ./gradlew assembleDebug     # green
  ./gradlew assembleRelease   # artifact under test (embedded JS bundle)
  ```

- Artifact: `app-release.apk`, 94,343,254 bytes, SHA-256
  `a8a6bdfeb4f14d3cd71b34069b532726a7464234551fbf417fff24e8e5230bc1`,
  `applicationId com.openagents.app`, `versionName 0.5.2`, `versionCode 1`,
  `minSdk 24`, `targetSdk 36`. Local release variant signed with the
  generated debug keystore — a verification artifact only, never a
  distribution artifact (signed distribution stays CUT-26).
- The in-app drawer label "Bundle 2026-07-11.cut-16-native-attachments" is
  the hardcoded `BUNDLE_TAG` constant in
  `apps/openagents-mobile/src/screens/home-core.ts` at this commit, not
  evidence of an older bundle; the embedded JS was built from `1eb9f1a95f`.

## Emulator environment (this Mac; nothing newly installed)

The complete toolchain already existed locally; no SDK/AVD installation was
required for this run:

- SDK root `/opt/homebrew/share/android-commandlinetools` (Homebrew
  `android-commandlinetools`): platform-tools 37.0.0, platforms android-35 +
  android-36, build-tools 34/35/36, emulator **36.6.11**, system image
  `system-images;android-35;google_apis;arm64-v8a`.
- JDK: Homebrew OpenJDK **17.0.19** (`JAVA_HOME` pointed at
  `/opt/homebrew/opt/openjdk@17/...`).
- AVD: pre-existing **`khala_test`** (Pixel 7 profile, `google_apis`,
  arm64-v8a) booted headless:
  `emulator -avd khala_test -no-window -gpu swiftshader_indirect -no-snapshot -no-audio -no-boot-anim`.
- Device facts from `adb shell getprop`: Android **15** (API 35),
  `sdk_gphone64_arm64`, abi `arm64-v8a`.
- Fresh-state discipline: an older `com.openagents.app` install existed on
  the AVD (firstInstallTime 2026-07-10); it was `adb uninstall`ed and the
  artifact above freshly installed (firstInstallTime 2026-07-12 01:10:04
  local) so no stale secure-store/SQLite state could leak into these
  journeys.

## Journey → evidence table

Screenshots live in
[`receipts/2026-07-12-android-emulator/`](./receipts/2026-07-12-android-emulator/).
All timestamps below are the emulator's local clock (CDT). No screenshot
contains credentials; the GitHub form was captured empty and no sign-in was
performed.

| # | Journey (CUT ref) | Commands | Result | Evidence |
| --- | --- | --- | --- | --- |
| 1 | Fresh install + cold launch (CUT-14 baseline) | `adb uninstall com.openagents.app` → `adb install app-release.apk` → `am start -n com.openagents.app/.MainActivity` | PASS — `Displayed …MainActivity +487ms`, `ReactNativeJS: Running "main"`, expo-sqlite native lib loads, home renders the Khala local surface | `01-cold-launch-signed-out.png`; logcat excerpt A |
| 2 | OTA posture on launch | (same launch, network up) | PASS — expo-updates checked `updates.openagents.com` channel `openagents-production`, `CheckCompleteUnavailable` / "No update available"; the locally built embedded bundle is what ran | logcat excerpt A |
| 3 | Drawer + signed-out surfaces | tap Menu; tap surface pill | PASS — drawer (New chat / thread / Settings / bundle tag) and the OpenAgents surface showing sync phase `local_ready`: "Local device ready … Link OpenAgents only for cross-device Sync" + `Link OpenAgents account` | `02-drawer-signed-out.png`, `03-openagents-surface-signed-out.png` |
| 4 | PKCE sign-in entry (CUT-14 auth precondition) | tap `Link OpenAgents account` | PASS (entry only) — Chrome Custom Tab opens `auth.openagents.com/authorize` → GitHub shows "Sign in to GitHub to continue to **OpenAgents**". Proves the Android AuthRequest → OpenAuth → GitHub handoff end-to-end up to the credential form. No credentials entered. | `04-pkce-signin-browser.png` |
| 5 | Sign-in cancellation path | tap ✕ on the Custom Tab | PASS — app returns to foreground in the prior phase (`local_ready`), matching the `cancelled → previousPhase` contract in `signInNativeSession`; no crash | `05-pkce-cancelled-return.png` |
| 6 | Deep link while running, signed out → fail closed (CUT-14 "reject unauthorized/stale targets") | `am start -W -a android.intent.action.VIEW -d "openagents://coding/session/session-fake-01?repository=repository-fake-01&thread=thread-fake-01"` | PASS — intent delivered to the running singleTask `MainActivity` (`Status: ok`); no navigation occurs, no session opens, no crash (`native-coding-target-delivery` holds unauthorized targets out of navigation) | `06-deeplink-signed-out-failclosed.png`; logcat excerpt B |
| 7 | Process death → cold start VIA deep link, signed out → fail closed (initial-URL path) | `am force-stop com.openagents.app` (pidof confirms dead) → same VIEW intent | PASS — `LaunchState: COLD` (+319ms); app boots to the default local home, does not open the target, no crash. `Linking.getInitialURL` path is exercised and fails closed | `07-deeplink-cold-start-failclosed.png` |
| 8 | Process death → plain relaunch (CUT-14 process-death leg, signed-out half) | `am force-stop` → `am start` | PASS — `Displayed +143ms`, JS boots, expo-sqlite reopens, state reconverges to the identical signed-out home (deterministic: journeys 1/7/8 all landed on the same state) | `08-process-death-relaunch.png` |
| 9 | Network-severed cold launch (offline behavior) | `svc wifi disable && svc data disable` → force-stop → relaunch | PASS — expo-updates fails with a typed `CheckError` ("Failed to download remote update") and falls back to the embedded bundle; app boots normally offline; no crash. Network re-enabled afterwards | `09-offline-cold-launch.png`; logcat excerpt C |

### Logcat excerpts (public-safe, trimmed)

Excerpt A — fresh cold launch (journey 1/2):

```text
01:10:10.684 I/dev.expo.updates: Updates state change: StartStartup …
01:10:10.943 I/ActivityTaskManager: Displayed com.openagents.app/.MainActivity for user 0: +487ms
01:10:11.103 I/ReactNativeJS: Running "main"
01:10:11.107 D/nativeloader: Load …/base.apk!/lib/arm64-v8a/libexpo-sqlite.so … ok
01:10:12.292 I/dev.expo.updates: Updates state change: CheckCompleteUnavailable …
01:10:12.294 I/dev.expo.updates: UpdatesController onBackgroundUpdateFinished: No update available
```

Excerpt B — deep link into the running app, signed out (journey 6):

```text
01:13:39.751 I/ActivityTaskManager: START u0 {act=android.intent.action.VIEW
  dat=openagents://coding/... flg=0x10000000 cmp=com.openagents.app/.MainActivity}
  with LAUNCH_SINGLE_TASK … result code=3
```

(no subsequent navigation, no `FATAL`/`AndroidRuntime` lines; screen unchanged)

Excerpt C — offline launch (journey 9):

```text
01:15:54.335 E/dev.expo.updates: Failed to download remote update … UpdateFailedToLoad
01:15:54.341 I/dev.expo.updates: Updates state change: CheckError … {message=Failed to download remote update}
```

(app continued to `ReactNativeJS: Running "main"` on the embedded bundle)

## Honest gaps — what this build cannot yet prove on Android

None of the following is waived; each is the exact remaining Android gate.

1. **Authenticated CUT-14 legs are not exercisable without a real GitHub
   sign-in.** The greenfield app has no dev-credential bypass (correctly —
   the fail-closed session model in `src/auth/` only accepts a PKCE-verified
   OpenAuth session; agent tokens are rejected by
   `/api/mobile/auth/session`, verified 401 during this run). The seeded
   public-safe test account's GitHub password is owner-held (an existing
   NEEDS_OWNER item). Blocked legs: authorized repository/session directory
   listing, exact thread selection + process-death restoration, the
   loss-accounted offline cache line ("Coding cache · N repositories · M
   sessions hidden until reconnect"), cross-scope subscription fencing under
   a live catalog, and deep-link acceptance of an *authorized* target. The
   deterministic halves of all of these remain fixture-proven by the CUT-14
   suites (real-SQLite close/reopen, 77 tests at landing).
   **Unblock (one owner-touch, then automatable):** complete one GitHub
   sign-in as the seeded test account inside the emulator (or land a typed
   test-credential path); everything downstream is then scriptable.
2. **CUT-12 supervision interaction has no reachable surface in the
   greenfield Android app.** The CUT-12 mobile panel
   (`live-agent-graph-panel.tsx`, commit `21e9740ed9`) landed in the
   deprecated `clients/khala-mobile` client, not in
   `apps/openagents-mobile`. The product app under test exposes no agent
   hierarchy/inspect/focus UI yet, so no Android interaction receipt for
   CUT-12 can exist against the product app until that surface is ported.
   This is a code residual to record on #8692, additional to the
   authenticated-session gap above.
3. **Notification-response entry is wired but not exercisable.** The
   listener path exists in `src/app.tsx`
   (`addNotificationResponseReceivedListener` +
   `getLastNotificationResponseAsync` feeding the same bounded
   `native-coding-target-delivery` queue as deep links), but there is no way
   to produce a real notification carrying
   `openagents.mobile.coding_target.v1` data without an authenticated push
   or in-app scheduling source. Shell-posted notifications
   (`cmd notification post`) do not route through the app's response
   listener.
4. **Signed-out process-death restoration is trivially empty by design.**
   With no authenticated owner, there is no persisted target to restore, so
   journeys 7/8 prove crash-free deterministic reconvergence, not the "exact
   thread restoration" close criterion. That criterion stays open pending
   gap 1.

Prior Android-emulator receipts exist for the deprecated `khala-mobile`
client (`docs/khala-code/receipts/2026-07-07-qam-6-android-emulator-run.md`,
`docs/khala-mobile/2026-07-06-android-emulator-launch-smoke-receipt.md`);
they are outside the CUT program and the greenfield app, and are not counted
as CUT evidence.

## What this changes in the CUT ledger

- CUT-14 (#8694): Android leg advances from "no evidence exists" to
  "install/launch/PKCE-entry/deep-link-fail-closed/process-death-relaunch
  proven on emulator; authenticated restoration + authorized deep-link +
  offline cache line remain, unblocked by one owner GitHub sign-in (or a
  typed test-credential path)". Emulator satisfies the platform gate per the
  2026-07-12 owner decision; the remaining blocker is session material, not
  hardware.
- CUT-12 (#8692): the Android gate now has a named code residual — port the
  supervision panel from the deprecated client into `apps/openagents-mobile`
  before any Android interaction receipt is possible.
- CUT-27 (#8707) criterion 2: the "entirely-missing Android leg" is no
  longer entirely missing; this receipt is its first installment.
