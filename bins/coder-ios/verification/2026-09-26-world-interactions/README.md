# World interaction verification

Internal TestFlight **0.5.0 (46)** is confirmed `VALID` and `IN_BETA_TESTING`
in the [release receipt](testflight-build46.json). This single release includes
the ordered map, companion, and reactive-gate changes in issues #9720, #9721,
and #9722, plus connection corrections in #9718. See
[design and controls](../../../../docs/verse/world-interactions.md) and the
[connection assessment](../../../../docs/coder/verification/2026-09-26-mobile-connections.md).

## Current Rust and adapter checks

These checks exercise the current door integration, including the map and
companion consumers. Earlier checkpoint logs remain alongside them.

| Check | Result | Retained evidence |
| --- | --- | --- |
| Shared Verse library without desktop features | 159 passed | [Final shared suite](doors-verse-tests-final.log) |
| Mobile library after the final main merge | 61 passed, 1 manual host fixture ignored | [Final mobile suite](final-main-mobile-tests.log) |
| Verse and mobile all-target Clippy with warnings denied | Passed | [Final Clippy](doors-final-clippy.log) |
| Desktop-enabled door domain, HUD, and private preference-store tests | 9 passed | [Desktop door tests](doors-desktop-tests.log) |
| Desktop pointer admission with an occluded anchor and visible gate edge | 1 passed | [Desktop visible-edge regression](doors-desktop-visible-edge-test.log) |
| Mobile pointer admission with an occluded anchor and visible gate edge | 1 passed | [Mobile visible-edge regression](doors-mobile-edge-tap.log) |
| Route failure remains visible until the next explicit action | 1 passed | [Route-error regression](doors-route-error-test.log) |
| Android application and instrumentation compilation; native pinch tests | Compilation passed; 2 JVM tests passed | [Final Android checks](../2026-09-26-mobile-connections/android-final-compile-and-pinch.log) |

The ignored mobile test, `computers_live_tests::serve_a_host_for_a_device_run`,
is an explicit host fixture for a simulator or device session. It is not an
unreported passing test. The desktop checks are focused checks plus all-target
compilation; they do not claim that every desktop test ran in this final pass.

The shared checks cover all eight map destinations, bounded collision-aware
routes, manual cancellation, camera independence, and the Gym approach. The
door checks cover item compatibility, independent remembered keys, reset,
versioned persistence, bounded effects, picking, and pointer cancellation.
A remembered item does not resume navigation after relaunch.

## Native checkpoints

At the map and companion checkpoint, [four combined native tests passed](map-companion-pinch-native.log):
map destination selection and manual cancellation, map drag isolation, a real
companion tap, and pinch in both directions. That checkpoint preceded the door
integration; it is not final build-46 acceptance.

The [initial combined run](native-integration-initial.log) ran 21 tests:
18 passed, one live-host fixture skipped, and two assertions failed. The
[focused retest](native-integration-retest.log) passes all six tests: Computers
status/invitation flow, masked owner-key cancellation, gate choices and restart
persistence, map dragging, walking/manual cancellation, and world-relay
persistence/Leave. The [machine-readable summary](native-integration-retest.json)
retains the counts and simulator identity.

The two initial failures were stale UI assumptions. Computers no longer shows
a mobile SSH control. The map cancellation gesture started inside the new
gate item strip while passing Halo. The corrected test starts below it at
normalized `(0.15, 0.88)` and asserts that this point is outside the visible HUD.
Its destination, movement, and cancellation assertions remain intact.

The final merge preserves another contributor's optional push registration
and shared secret-field component. Push remains unconfigured for this release.
The first [merged-app pass](native-main-merge-initial.log) passed masking/cancel
and normal startup but exposed a keyboard/form integration failure. The final
[six-test pass](native-final.log), with its [summary](native-final.json), passes
wrong-key refusal and correct-key acceptance, masking/cancel, full-bleed layout,
both map checks, and normal launch/resume/relaunch. Physical HUD safe areas now
come from the window; keyboard insets belong only to the native panel.

An additional [normal-launch check](normal-launch.json) starts the optimized
app without synthetic arguments while external Rust dylibs are temporarily
unavailable. It remains alive after ten seconds. [Startup screenshot](normal-launch.png).

## Archived source and subsequent integration

The [archive receipt](archive-source.json) pins clean source commit
`a0426ded9d2c8762a7a4536237ed1e6dd558c0e5`, the Cargo lock digest, compiler
versions, and the executable digest. The [bundle check](archive-bundle-verification.json)
confirms system-only dynamic dependencies; strict code-signature verification
also passed. Push registration remains unconfigured in this archive.

After archiving, main gained independent computer-directory, SSH-route, and
Android push changes. The merged source passes the [mobile library suite](post-archive-main-mobile-tests.log)
(61 passed, one manual fixture ignored), plus [Android main/instrumentation
compilation and two JVM pinch tests](post-archive-android-compile.log). The
Android merge preserves both contributors' controls and moves the incoming
owner-key/push test navigation through Settings. These later source checks
do not change which commit was archived. Firebase-configured push and a new
Android emulator run are outside this release's acceptance.

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
- The [initial native integration failures](native-integration-initial.log)
  remain retained with their [summary](native-integration-initial.json). The
  corrected checks pass in the [six-test retest](native-integration-retest.log).

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
