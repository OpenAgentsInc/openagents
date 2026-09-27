# Motion camera correction — September 26, 2026

[Issue #9716](https://github.com/OpenAgentsInc/openagents/issues/9716) addresses
body turns that did not turn the view, restricted upward look, and abrupt
sensor-driven camera movement. The implementation is shared Rust camera logic
with thin iOS and Android motion adapters. No model or benchmark runs are part
of this verification.

## Causes and corrections

The old projection inverted the native attitude quaternion. Its synthetic
fixtures used the same inverse convention, so those tests agreed with the
implementation without proving that a physical body turn worked. The corrected
contract is a Hamilton quaternion from portrait device coordinates to a
reference whose vertical axis follows gravity. Rust rotates the back-of-phone
viewing direction into that reference, then extracts heading and pitch. Screen
roll does not tilt the horizon. Android now forwards its native rotation-vector
quaternion in the same convention instead of conjugating it.

Native fixtures independently check the convention through Core Motion's
`CMAttitude` rotation matrix and SIMD. An upright portrait phone has gravity
along its negative Y axis. Turning the body left or right preserves that gravity
vector while rotating the phone's viewing direction. Pointing upward raises the
viewing direction. These checks do not call the Rust projection and would
reject the previous fixtures. The decoder uses Core Motion archive keys in
test-only code; a future archive-format change fails the check explicitly.

The camera also clamped its eye above the ground and then aimed back at the
avatar's shoulders. That geometry flattened upward look regardless of the
requested pitch. The corrected view matrix preserves the requested direction
when ground clearance lifts the eye. Pitch now spans approximately 83 degrees
above and below the horizon. Ordinary downward orbit views still aim at the
avatar; the sky view can move the avatar out of the center of the screen.

Sensor input previously moved the camera immediately. It now updates a target;
each rendered frame approaches that target with an exponential response and a
60 ms time constant. Heading interpolation follows the shortest turn across
the angle boundary. The response depends on elapsed time, not frame count or
the simulation's capped physics step. iOS requests 60 Hz sensor and display
updates, with a 30 Hz display minimum. Fresh sensor timestamps are deduplicated;
an additional wall-clock delivery throttle does not discard frames when display
callbacks jitter around 60 Hz. Near a vertical phone orientation,
hysteresis freezes unstable heading while keeping pitch responsive. Heading
recenters when it becomes defined again. Recenter, panel transitions,
backgrounding, and long gaps discard the pending target.

A separate timing bug compared sample age with the previous rendered frame.
At a slow frame rate, a current sample could therefore appear stale. Both
adapters now send the sample timestamp and its actual receipt time in the
frame clock domain. Rust and the adapters accept readings up to 250 ms old or
5 ms ahead of receipt; duplicate, reordered, invalid, and stale readings remain
rejected. Android converts its sensor boot-clock timestamp to the frame clock
with a paired receipt reading. This addresses the motion admission defect
tracked in [#9714](https://github.com/OpenAgentsInc/openagents/issues/9714);
that issue's separate Android Gym fixture failure is outside this correction.

## Verification and delivery

The [evidence directory](../../../bins/coder-ios/verification/2026-09-26-motion-camera/README.md)
records the focused checks and release receipt. The current source build is
`0.5.0 (44)`. All 19 mobile scene tests, 124 portable Verse tests, strict
Clippy, desktop compilation, native Swift checks, Android Kotlin compilation,
and all three focused iOS integration tests passed. The first simulator run
was interrupted by another test session replacing its runner; the retained
evidence explains the failure and the final isolated run. Availability is
recorded only after App Store Connect confirms processing and internal
distribution.

Physical-device comfort, sensor noise, frame rate, and thermals require testing
on a phone. Simulator and deterministic native checks establish the corrected
mapping and application behavior, not a hands-on physical-device acceptance
result.
