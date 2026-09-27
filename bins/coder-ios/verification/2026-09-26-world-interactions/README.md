# World interaction verification

The confirmed internal TestFlight release remains **0.5.0 (45)**. Source for
build 46 adds the ordered map, companion, and reactive-gate changes in
issues #9720, #9721, and #9722. Combined native acceptance and distribution are
pending. See [design and controls](../../../../docs/verse/world-interactions.md)
and the separate [connection corrections](../../../../docs/coder/verification/2026-09-26-mobile-connections.md).

## Current Rust and adapter checks

These checks exercise the current door integration, including the map and
companion consumers. Earlier checkpoint logs remain alongside them.

| Check | Result | Retained evidence |
| --- | --- | --- |
| Shared Verse library without desktop features | 159 passed | [Final shared suite](doors-verse-tests-final.log) |
| Mobile library after the visible-edge correction | 60 passed, 1 manual host fixture ignored | [Final mobile suite](doors-final-mobile-tests.log) |
| Verse and mobile all-target Clippy with warnings denied | Passed | [Final Clippy](doors-final-clippy.log) |
| Desktop-enabled door domain, HUD, and private preference-store tests | 9 passed | [Desktop door tests](doors-desktop-tests.log) |
| Desktop pointer admission with an occluded anchor and visible gate edge | 1 passed | [Desktop visible-edge regression](doors-desktop-visible-edge-test.log) |
| Mobile pointer admission with an occluded anchor and visible gate edge | 1 passed | [Mobile visible-edge regression](doors-mobile-edge-tap.log) |
| Route failure remains visible until the next explicit action | 1 passed | [Route-error regression](doors-route-error-test.log) |
| Android application, instrumentation sources, and JVM task | Build successful; unchanged JVM task reused | [Android door compilation](../2026-09-26-mobile-connections/android-doors-compile.log) |

The ignored mobile test, `computers_live_tests::serve_a_host_for_a_device_run`,
is an explicit host fixture for a simulator or device session. It is not an
unreported passing test. The desktop checks are focused checks plus all-target
compilation; they do not claim that every desktop test ran in this final pass.

The shared checks cover all eight map destinations, bounded collision-aware
routes, manual cancellation, camera independence, and the Gym approach. The
door checks cover item compatibility, independent remembered keys, reset,
versioned persistence, bounded effects, picking, and pointer cancellation.
A remembered item does not resume navigation after relaunch.

## Native checkpoints and pending combined acceptance

At the map and companion checkpoint, [four combined native tests passed](map-companion-pinch-native.log):
map destination selection and manual cancellation, map drag isolation, a real
companion tap, and pinch in both directions. That checkpoint preceded the door
integration; it is not final build-46 acceptance.

The final combined simulator build succeeded. Its native suite is still under
review. The map manual-cancellation test reached its destination-selection and
walking assertions, but its old drag-start coordinate overlapped the new gate
item strip while passing Halo. The strip correctly owns a contact that starts
inside it. The test now starts below the strip at normalized `(0.15, 0.88)` and
asserts that this point is outside the visible HUD before dragging. Its route
and movement assertions remain intact. A retest is required before declaring
the combined suite passed. Door acceptance, archive, upload, and processing receipts
must be added here as they become available.

## Retained failures and corrections

- The [first map and pinch run](map-and-pinch-native.log) passed two tests and
  failed the zoom-out assertion. Its contact landed on the native Recenter
  control. The corrected clear-surface gesture passed in both directions in
  the [combined checkpoint](map-companion-pinch-native.log); the
  [connection evidence](../2026-09-26-mobile-connections/README.md) retains the
  input diagnosis and screenshots.
- The [first shared door suite](doors-verse-tests.log) passed 158 tests and
  failed `world::tests::faces_are_near_black`. Amber gate-title faces had been
  added to the static world mesh, violating its renderer contract. The titles
  moved into bounded, depth-tested dynamic geometry. The
  [corrected shared suite](doors-verse-tests-final.log) passed all 159 tests.
- The final native map assertion described above remains unresolved in this
  receipt until its corrected-coordinate retest is retained. Passing earlier
  checkpoints does not erase that integration failure.

## Visual evidence

- [Expanded map with eight destinations](map-eight-landmarks.png).
- [Spark lightning](spark-lightning.png) and [Halo rings](halo-rings.png) are
  frames extracted from the simulator acceptance video. Portions of the gates
  are clipped by the camera frame; these are effect observations, not a claim
  that the entire gate is visible from every nearby viewpoint.

## Limits

Simulator and compilation results do not establish physical-device motion,
performance, battery use, or release acceptance. No model or benchmark calls
were run for these interactions. The gates are local navigation demos: they do
not execute services, spend funds, synchronize an inventory, or grant access.
