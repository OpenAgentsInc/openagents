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

## Final native acceptance

All three targeted tests passed in 107.6 seconds. See the
[final summary](native-final-summary.json) and [log](native-final.log).

- The Gym board opens from its projected screen coordinates. The test inspects
  chart data, backgrounds and resumes the app, closes the details, leaves the
  interior, and confirms that Gym observation stops. No native entry button is
  present. The [board](native-gym-board.png), [chart](native-gym-chart.png), and
  [exit](native-gym-exit.png) screenshots were visually inspected. The board
  screenshot is a close-up of the actual world geometry.
- The storage test starts with an absent preview preference, confirms no relay
  override is passed to Rust, joins a custom relay in network-inert preview mode,
  and verifies that it survives panel closure and app relaunch. **Leave** writes
  an empty value; after another relaunch, Rust receives `world_offline: true` and
  the app remains offline. The [saved-relay image](native-saved-relay.png) records
  the custom choice.
- The production test launches without synthetic mode. With no saved relay
  preference and no native relay override, Rust connects to
  `wss://relay.openagents.com`. It then reports live remote entities and nonzero
  remote geometry submitted in a rendered frame.

The retained [production observation](native-public-world-observation.json)
records 2 live remote entities, 10 retained remote entities, 10,000 submitted
remote vertices, and 2,414 presented frames. Entity counts include avatars and
companions; they are not a count of people. The public key in that observation
lets the independent relay witness identify the phone without access to its
secret key. See the separate [relay evidence](../2026-09-27-relay-presence/README.md).
The initial [public-world screenshot](native-public-world.png) does not clearly
show the test peer, which was outside the narrow portrait camera view. Geometry
submission and signed relay evidence are recorded independently; this image is
not presented as visual proof of a visible peer.

The [attachment manifest](native-attachments.json) binds the native screenshots
and observation to their original tests and timestamps. The production app was
kept active after the test so the witness could record its subsequent signed
idle frames. The witness uses a separate fresh peer identity, not an injected
native fixture.

Xcode reports an unavailable LLDB debugger version and skips App Intents metadata
extraction because the app does not depend on `AppIntents.framework`. The tests
continue and pass. No application crash or rendering error was observed.

## Visible peer follow-up

A second independent peer repeated the public-relay check with its avatar
positioned 0.8 meters to the side and 3 meters ahead of the phone's verified pose.
Only the verification example's offset changed; the same installed build 48 app
remained active. The [new native screenshot](native-visible-peer.png) clearly
shows a separate avatar and its companion ahead of the local player. The
[capture receipt](native-visible-peer-receipt.json) records the image digest,
time, installed build, phone key, and peer key. The associated
[independent witness](../2026-09-27-relay-presence/phone-peer-visual.json) records
signed relay traffic and cleanup separately.

The earlier screenshot and counts remain retained. The visible-peer follow-up
supplements them; it does not relabel retained offline entities as live peers.


## Targeted code checks and distribution

The [targeted check receipt](targeted-checks.json) retains 67 passing mobile
library tests with one manual fixture ignored, 215 passing portable Verse tests,
mobile all-target Clippy with warnings denied, and Android Kotlin compilation.
The retained command logs keep their original whitespace. No benchmark or
model run was started, and the full workspace release gate was not required.

The [archive receipt](archive-source.json) pins clean source
`cb655ee1ff4282f30f1a25f2adda8f08d7f85fa1`. Signature and bundled-library checks
passed; the app includes no forest pack and leaves notification wakes disabled.
The later verifier-only peer offset change does not enter the application.

Apple accepted **0.5.0 (48)** with `VALID` processing and `IN_BETA_TESTING`
internal availability. The [distribution receipt](testflight-build48.json)
records the upload, release notes update, and archived source. The independent
probe processes exited, and the test app on the dedicated simulator was stopped
after verification. Phone leave delivery is not inferred from that termination.
