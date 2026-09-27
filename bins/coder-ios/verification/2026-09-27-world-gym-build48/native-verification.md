# Build 48 native verification

The checks use Coder 0.5.0 (48), Release Swift, optimized Rust, and the dedicated
`Coder Launch 45` iPhone 17 Pro simulator on iOS 26.5. The build keeps push
disabled. Simulator checks do not establish physical-device acceptance.

## First native batch

Both Gym tests and the normal production startup, background/resume, and relaunch
test passed. The Gym tests tap the projected GPU board, inspect the detailed
chart, leave the interior, and exercise the synthetic recipe confirmation. The
preview rejects execution; no training or evaluation run starts.

The relay preference test failed because its test input cleared text from the
middle of the URL and left the suffix `com`. The app correctly preserved the
resulting URL, `wss://world.example.testcom`. The test now uses the native
**Select All** command and verifies the exact replacement before tapping **Join**.
The original [log](native-first-attempt.log) and
[summary](native-first-attempt-summary.json) retain the three passes and one
failure.

## Final optimized build

The final [build log](native-release-build.log) includes the typed live remote
entity count, the successful-read storage-error reset, and passive native
observation metadata. `--uitest-observe-world` exposes bounded public world
metadata for acceptance checks. It does not select a synthetic identity, change
relay preferences, inject avatars, or enable another transport. The live test
is opt-in with `TEST_RUNNER_CODER_WORLD_LIVE_TEST=1` and requires an independent
relay witness.

The synthetic-only preference reset removes only the preview Keychain item.
It lets the storage test distinguish an absent item from an empty value that
records **Leave**. Production settings are not reset by that fixture.
