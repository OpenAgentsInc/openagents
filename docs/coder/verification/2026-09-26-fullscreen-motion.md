# Full-screen Verse and motion look — September 26, 2026

[Issue #9701](https://github.com/OpenAgentsInc/openagents/issues/9701) extends
Coder's iOS Verse home with a full-screen Metal canvas and optional phone-motion
camera control. The [mobile guide](../../verse/mobile.md) describes the controls.

## Layout contract

The world occupies the complete application window, including the areas behind
the system status bar and home indicator. The system clock remains the iOS
clock; the application does not draw a duplicate. Native controls use the safe
area above the canvas so they remain reachable. World interaction markers use
the complete Metal viewport for projection, including the top and bottom edges.
Computer and Gym panels remain anchored to the world.

## Camera contract

Touch look uses the right side to turn the camera. Motion look reads device
orientation through a thin Core Motion adapter and sends bounded samples to
Rust. Rust owns the selected mode, relative orientation, camera bounds, movement,
and lifecycle. Holding the left side moves forward in motion mode, while the
phone's orientation changes the view and movement heading. Recenter chooses a
new comfortable phone orientation without jumping the current world view.

Motion sampling is active only when needed for foreground world interaction.
Backgrounding, opening an in-world panel, returning to touch look, or disposing
the surface stops sampling. Resuming establishes a new baseline. Missing or
failed sensor access leaves a clear touch fallback. Roll does not tilt the
world horizon. This product behavior stays outside the generic Rust Native crate.

The adapter uses Apple's [processed device-motion API](https://developer.apple.com/documentation/coremotion/getting-processed-device-motion-data)
and [attitude representation](https://developer.apple.com/documentation/coremotion/cmattitude).
The selected gravity-aligned reference tracks relative orientation without
requiring a geographic heading. Raw samples are local inputs to the Rust camera;
this change does not publish an inertial-sensor stream.

## Evidence and limits

All 24 mobile library tests and strict all-target Clippy pass. The integrated
iOS simulator suite passes all 12 tests, including the existing reader, Gym,
and world workflows. After the final motion refinements, a fresh build passes
the three full-screen and motion tests against unchanged source hashes.

The native layout check measures the Metal surface and application window at
the same rectangle: `[0, 0, 402, 874]` points. The visible system status bar
occupies `[0, 0, 402, 54]` within that canvas. The retained screenshot also shows
world geometry behind the clock. The injected-motion check covers left-hold
movement, right-touch exclusion, recentering, background/resume, and returning
to touch look. Separate adapter checks verify bounded sampling and shutdown.

Verification and distribution receipts are recorded in the
[full-screen and motion evidence directory](../../../bins/coder-ios/verification/2026-09-26-fullscreen-motion/README.md).
The native simulator uses explicitly labeled synthetic motion input to exercise
camera behavior because it does not supply a physical phone's inertial sensors.
Rust tests check camera math and lifecycle separately from native layout and
input tests. A successful device archive verifies compilation and signing;
it does not prove motion comfort, drift, battery use, or sensor behavior on a
physical iPhone. No model, training, or benchmark workload is part of this change.

## Distribution

Coder **0.5.0 (42)** is available in internal TestFlight. Apple reports `VALID`
and `IN_BETA_TESTING`. The archive uses clean source
[`f7389bb05f`](https://github.com/OpenAgentsInc/openagents/commit/f7389bb05f6849d717392a310c20169f16f0390d)
and passed strict code-signature verification. The
[release receipt](../../../bins/coder-ios/verification/2026-09-26-fullscreen-motion/testflight-build42.json)
records the source, lockfile, executable, toolchain, and Apple build identity.
The bundle, signing team, version 0.5.0, and profile remain unchanged. This is
an internal beta; physical motion direction and comfort still need a phone test.
