# Full-screen Verse and motion-look evidence

This evidence covers [issue #9701](https://github.com/OpenAgentsInc/openagents/issues/9701).
See the [implementation assessment](../../../../docs/coder/verification/2026-09-26-fullscreen-motion.md)
and [mobile controls](../../../../docs/verse/mobile.md).

The native checks mount the actual Metal surface and assert that its screen
rectangle matches the application window, including the area behind the system
clock and home indicator. Screenshots record the full-screen world and controls.
Native motion fixtures are enabled only in explicitly synthetic test launches;
they are not physical-device measurements.

Rust checks cover relative orientation, yaw and pitch bounds, stale samples,
mode switches, movement, and lifecycle. Native checks cover control wiring,
full-screen layout, unavailable-motion fallback, and existing computer/Gym
interactions. The device archive and TestFlight receipt are separate evidence
for compilation, signing, and distribution.

No model, training, or benchmark run is performed. A physical iPhone motion
session, sensor drift, thermal behavior, battery use, and comfort remain outside
simulator acceptance. The reference pose is relative to the phone's current
orientation; it is not geographic compass navigation.

## Results

| Check | Result | Evidence |
| --- | --- | --- |
| Rust mobile library | 24 passed, including 13 world/camera tests | [Rust receipt](rust/receipt.json), [test log](rust/full-tests.log) |
| Strict all-target Clippy | Passed | [Clippy log](rust/clippy.log) |
| Native motion adapter and iOS 17 typecheck | Passed | [Adapter receipt](adapter/receipt.json) |
| Full native reader/Gym/world suite | 12 passed before the final motion refinements | [Native receipt](native-receipt.json), [summary](native-full-suite-summary.json), [compressed log](native-full-suite.log.gz) |
| Rebuilt final full-screen/motion suite | 3 passed; all 1,107 captured source inputs unchanged during the run | [Final source](native-final-source.json), [summary](native-final-summary.json), [compressed log](native-final-tests.log.gz) |
| Metal canvas geometry | Surface and window both `[0, 0, 402, 874]` points; status bar covered | [Geometry](native-frame-geometry.json) |

The native receipt identifies the five source files that changed between the
first and final passes. Final motion acceptance uses the rebuilt executable.
The [full-screen capture](native-fullscreen.png) shows building geometry behind
the system clock. The [motion capture](native-motion.png) shows the mode and
recenter controls after injected orientation and movement; the simulator
compositor omits some unchanged clock and Sprint glyphs in that capture.
The [unavailable-motion capture](native-motion-unavailable.png) records the
explicit touch fallback. None of these captures contains private chats.
