# Android parity evidence plan (G1, #12103)

This file is the evidence ledger for the Android parity matrix in
[the sprint plan](2026-10-18-android-parity-sprint.md). Code completion and
device parity are separate claims. This sandbox has no emulator, Android SDK,
or FCM access, so nothing below is marked passed. Each row names the journey to
run and where its artifacts go. Fill in the run record when the check runs.

## Run record (one per candidate build)

| Field | Value |
| --- | --- |
| Baseline commit (iOS/shared core) | `f4d84c49ce864e748289772be1029e8b7b382e7e` |
| Candidate commit | |
| Build ID (`versionCode`, APK SHA-256) | |
| ABI / API level / device | arm64-v8a and x86_64; API 26 and API 35; emulator image |
| Commands | `apps/openagents/scripts/android/app.sh check`, then the journeys below |
| Failures | |
| Artifact directory | `$(openagents scratch)/android-parity/<candidate>/` |

Design Lab captures: for each surface, a light and a dark PNG plus the
UI-automator tree dump (`uiautomator dump`), named
`<surface>.<light|dark>.<widthdp>x<heightdp>.{png,xml}`. Record the platform,
density, and font scale beside each capture.

## Journey per matrix row

All journeys run offline on an isolated emulator, without the owner's account.
A live step uses the dedicated test account with `OPENAGENTS_TEST_RUN=1` and
removes what it created.

| Row | Journey | Existing automated check | Status |
| --- | --- | --- | --- |
| Start and identity | Cold start, first chat with no sign-in, tutorial once, back up and restore Your ID | `AndroidCoreBootstrapTest` | Not run on device |
| Chats and navigation | Search, pin, reorder, archive; airplane mode and reconnect; selected chat stays | `DrawerPresenterTest` | Not run on device |
| Transcript | Rich markdown, code, tables, images; select and copy; stream and keep scroll anchor | `AndroidTranscriptTextTest` (androidTest) | Not run on device |
| Session controls | Send, queue, edit, steer, stop; approval card; IME open and close | none yet | Missing |
| Projects and computers | Create project, Run on, cloud and host handoff | none yet | Missing |
| Agents and Home | Create, save, run an agent; Agent Lab; Home widgets | none yet | Missing |
| Visuals and Labs | Video, compare, 3D, report cards | `RustNativeLabCardBridgeTest`, `LabSurfaceResourcesTest`, `VisualFrameMailboxTest` | JVM only |
| Settings and account | Devices, appearance, share, notifications, delete account | none yet | Missing |
| Wallet and upgrades | Balance, receive, send, retired seed kept after in-place upgrade | none yet | Missing |
| Scanner and links | Pair, sign-in approval, project invitation, App Link | `AndroidScannerPresentationBoundaryTest` | Not run on device |
| Notifications | Push and nopush builds; cold and warm tap opens the right chat | `AndroidPushCoordinatorTest` | Not run on device; FCM needs a device |
| Voice | Routes, interruption, minimize, mute from notification | none yet | Missing |
| Android access and lifecycle | TalkBack focus order, 200% font, reduced motion, process death restores draft | none yet | Missing |
| Adaptive navigation | Rotate and resize phone to tablet and split with the same chat, draft, anchor | `AdaptiveNavigationPolicyTest` | JVM only |
| Build and distribution | Both ABIs, 16 KiB alignment, signature and version continuity | `scripts/android/check-release-apk.sh`, `verify-release-artifacts.py` | Not run here |

## Budgets

Numbers come from the first baseline run on the same emulator image, not from
a guess. Until a baseline exists, a row is "unmeasured" and parity is not
claimed for it. Budget rule: the candidate may be at most 10% worse than the
baseline on time and memory, and must match it exactly on text wrapping.

| Metric | How to measure | Baseline | Candidate | Budget |
| --- | --- | --- | --- | --- |
| Cold start to first chat | `am start -W` (`TotalTime`), median of 10 | unmeasured | | baseline + 10% |
| Text and wrapping accuracy | Rust row positions vs Canvas origins, mixed scripts | unmeasured | | 0 mismatches |
| Streaming | Time from delta to painted row, p95 | unmeasured | | baseline + 10% |
| Scroll frame pacing | `dumpsys gfxinfo` janky frames over a 5,000-row scroll | unmeasured | | baseline + 10% |
| Keyboard | Time from focus to composer settled | unmeasured | | baseline + 10% |
| Memory | `dumpsys meminfo` PSS after 1,000 rows | unmeasured | | baseline + 10% |
| Library and package size | Native `.so` per ABI and APK size | unmeasured | | baseline + 10% |
| Process recovery | `am kill`, relaunch: identity, grants, draft kept | unmeasured | | all kept |

## Remaining evidence and owners

These are not done and do not hold the issue open.

- Emulator journeys and measurements above: Android lane (#12100 integrator).
- Physical-device run, FCM delivery, and a real voice call: owner (`NEEDS_OWNER.md`).
- Play internal-testing upload and signature continuity: release agent / owner.
- Integration blockers recorded on #12103 (voice service manifest entry,
  onboarding source roots, Cloud/NewCloud routing, regenerated UniFFI
  bindings): #12100.
