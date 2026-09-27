# Android slow-frame acceptance — September 27, 2026

This record closes
[#9714](https://github.com/OpenAgentsInc/openagents/issues/9714): Android motion
look and the Gym acceptance test under slow rendering. It covers the Gym
fixture diagnosis and fix, and the Android re-runs of both affected native
tests. The motion admission correction landed earlier in `c888f72e1f`
(#9716); see the [motion camera record](2026-09-26-motion-camera.md).

## Evidence class

Android emulator only, on this Mac (Apple silicon). No physical device, no
relay, no model, and no benchmark run was involved. Every test used the
isolated synthetic state the instrumentation suite sets up.

## Gym fixture: cause and fix

In synthetic mode, `verse::gym::Board::set_active` applied the fixture
snapshot once, stamped with the entry time. A real board's worker refreshes
every `REFRESH` (5 seconds), but the synthetic board never did. `poll` marks
observations stale 30 seconds after the last snapshot, so on a slow emulator
the test reached the recipe button after the preview had gone stale, and the
stale-snapshot refusal fired as designed.

`d9f8a1265e` refreshes the synthetic snapshot on the host's interval while
the player is inside. The staleness check is unchanged:

- `the_synthetic_preview_refreshes_like_a_host_while_inside` sets the last
  snapshot 31 seconds in the past, polls, and expects a fresh, selectable
  recipe. It fails without the fix.
- `a_real_board_without_fresh_snapshots_goes_stale` keeps the refusal
  enforced for a real board that stops receiving snapshots.
- The existing `stale_and_changed_recipes_cannot_be_confirmed` still passes.

`cargo test -p verse --lib` passed 143 tests; Clippy with `-D warnings` and
formatting were clean.

## Android runs

Source: `main` at `fa6ed50719`, which includes both `c888f72e1f` and
`d9f8a1265e`. The emulator was a copy of the `coder_mobile_api35`
configuration (API 35, ARM64, Google APIs, 2 GB) under a separate AVD name,
`oa_9714_api35`, so the shared AVD stayed free. It was deleted afterwards.

| Configuration | What ran | Result |
| --- | --- | --- |
| `-gpu swiftshader`, default cores | Full synthetic suite, `CODER_ANDROID_SERIAL=emulator-5580 scripts/build-coder-android.sh test` | 24 tests: 22 passed, 2 skipped by design (the live Computers test and the push-configured test need external setup), 0 failed. |
| `-gpu swiftshader`, default cores | `motionModeRecenterAndPauseUseTheSharedController` and `gymHasSyntheticBoardsWithoutLaunchingWork`, 3 runs | 3 of 3 passed; about 15 seconds a run. |
| `-gpu swiftshader`, `-cores 1`, six busy loops in the guest | The same two tests, 3 runs | 3 of 3 passed; 76, 220, and 83 seconds a run, well past the 30-second staleness bound and slower than the original failing run. |
| `-gpu host` | The same two tests, then the full suite | Not runnable: the renderer reports that the host GPU adapter supports no storage buffers, storage textures, or compute limits, which the Verse scene needs. 15 of 24 tests failed at launch with that renderer error; this is an environment limit, not the motion or Gym behavior under test. |

## Limits

- No hardware-accelerated emulator can run the Verse scene on this Mac, so
  every passing run is software-rendered. Neither establishes physical-device
  performance.
- The slow-rendering load was induced with CPU contention, not a measured
  frame rate. The run times show the scenario the issue describes: a session
  much longer than the staleness bound.
- Physical-device acceptance remains separate.
