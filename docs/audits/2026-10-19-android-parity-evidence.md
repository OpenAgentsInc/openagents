# Android parity evidence plan (G1, #12103)

This file is the evidence ledger for the Android parity matrix in
[the sprint plan](2026-10-18-android-parity-sprint.md). Code completion and
device parity are separate claims. The first emulator run against production
test accounts was on 2026-10-19; its journey table, defects, and fix commits are
in [the mobile 1.0.0 parity run](2026-10-19-mobile-1.0-parity-run.md). Rows that
run still lacks stay open below.

## Run record (one per candidate build)

| Field | Value |
| --- | --- |
| Baseline commit (iOS/shared core) | `f4d84c49ce864e748289772be1029e8b7b382e7e` |
| Candidate commit | `67ac968f7b` (release 1.0.0, versionCode 1016) |
| Build ID (`versionCode`, APK SHA-256) | 1016, `a7fc797f505db2ec2a8167dedc5ee78d88b1cc3a59d0ae15aa6526f3e48aed46` |
| ABI / API level / device | arm64-v8a, API 35, Pixel 7 emulator (API 26 not run) |
| Commands | `apps/openagents/scripts/android/app.sh check`, then the journeys below |
| Failures | Nine defects found and fixed; see the parity run |
| Artifact directory | `~/work/releases/android/1.0.0-1016/`; Design Lab captures in `~/work/design-lab/captures/android/` |

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
| Start and identity | Cold start, first chat with no sign-in, tutorial once, back up and restore Your ID | `AndroidCoreBootstrapTest` | Cold start, tutorial, and first account passed on emulator; backup and restore not run |
| Chats and navigation | Search, pin, reorder, archive; airplane mode and reconnect; selected chat stays | `DrawerPresenterTest` | Search, list, and live refresh passed on emulator; pin, reorder, archive, airplane mode not run |
| Transcript | Rich markdown, code, tables, images; select and copy; stream and keep scroll anchor | `AndroidTranscriptTextTest` (androidTest) | Markdown, code, and streaming rendered on emulator (14 demo sessions); select and copy not run |
| Session controls | Send, queue, edit, steer, stop; approval card; IME open and close | none yet | Send passed on emulator (cloud, saved, live chats); queue, edit, steer, stop, approval not run |
| Projects and computers | Create project, Run on, cloud and host handoff | none yet | Project invitation join, shared project chats, computers in Settings passed on emulator; create project, Run on, handoff not run |
| Agents and Home | Create, save, run an agent; Agent Lab; Home widgets | none yet | Missing |
| Visuals and Labs | Video, compare, 3D, report cards | `RustNativeLabCardBridgeTest`, `LabSurfaceResourcesTest`, `VisualFrameMailboxTest` | JVM only |
| Settings and account | Devices, appearance, share, notifications, delete account | none yet | Settings, devices, appearance, project sharing passed on emulator; notifications and delete account not run |
| Wallet and upgrades | Balance, receive, send, retired seed kept after in-place upgrade | none yet | Missing |
| Scanner and links | Pair, sign-in approval, project invitation, App Link | `AndroidScannerPresentationBoundaryTest` | Project invitation App Link passed on emulator; pair and sign-in approval not run |
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
