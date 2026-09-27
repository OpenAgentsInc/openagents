# Forest zone verification

The shared implementation in `15f1c90cc3` adds the runtime-loaded forest,
portal, and bounded SRD 5.1 encounter for issues #9725, #9726, and #9727.
See [zone behavior and architecture](../../../../docs/verse/zones.md),
[exact rules coverage](../../../../docs/verse/zone-rules.md), and
[artwork provenance](../../../../assets/verse/forest/README.md).

Build 47 passed its native portal, download, encounter, return, and startup
checks. Distribution is pending; a source commit or simulator check does not
establish Apple processing or TestFlight availability.

## Targeted checks

| Check | Result | Evidence |
| --- | --- | --- |
| Verse all targets, including desktop library and local relay integrations | 230 library tests and five integration tests passed | [Verse tests](verse-all-target-tests.log) |
| Final asset loader/cache tests after cache cleanup correction | 10 passed | [Cache tests](asset-cache-tests.log) |
| Final shared HUD contact regression | Two passed | [HUD tests](hud-contact-regression.log) |
| Mobile library | 65 passed; one manual live-host fixture ignored | [Mobile tests](mobile-tests.log) |
| Verse/mobile all-target Clippy, warnings denied | Passed on final source | [Clippy](verse-mobile-clippy.log) |
| Android host lint and JVM tests | Passed | [Android checks](android-checks.log) |
| Android arm64 JNI library and APK | Built; all four ELF LOAD segments and APK checked for 16 KiB alignment | [Package log](android-native-package.log), [artifact receipt](android-native-package.json) |

The full Verse suite preceded the final cache-hit cleanup and unchanged-layout
HUD correction. Their focused suites and the final all-target Clippy passed
afterward. The ignored mobile test is the pre-existing opt-in live-host fixture;
it is not counted as passing. No model or benchmark runs were performed.

The loader tests cover exact hashes and byte counts, corrupt or oversized
geometry, cancellation, one-worker admission, cache atomicity, no-follow opens,
and pruning that preserves unrelated files. Mobile tests cover the real pack,
portal occlusion, cancellation without a rendered frame, held-input reset,
plaza session teardown/restart, and exact return position. Rules tests inject
die faces and verify the declared subset, including refusal atomicity.

## Native iPhone acceptance

The [native verification record](native-verification.md) retains both attempts
and their limitations. The final five-test batch passes startup/resume/relaunch,
map input, the portal/encounter/return flow, and bundled notices. It starts with
no forest cache, downloads the published pack over HTTPS, and independently
checks its exact size and digest. The normal optimized launch runs without
synthetic arguments or external Rust dylibs and creates no forest cache.

The first attempt reached the forest but failed three checks: a renamed startup
accessibility value, an incorrect compact-map zoom assertion, and SwiftUI
replacing the nested credits accessibility identifier. The final test corrects
the stale assumptions and moves the disclosure identifier to its label. The
original failures remain in the native record. A final one-test cached traversal
also passes after shortening the idle caption and reusing the shared text
wrapper so portrait captions break at spaces. Final [forest](native-forest-final.png)
and [encounter](native-encounter-final.png) screenshots show that correction.

## Visual review

The [shared-renderer forest capture](forest-shared-render.png) uses the verified
pack and the same renderer, scene, camera, and HUD as the native hosts. An early
capture exposed arrival behind the return arch; the final arrival is eight
meters into the path with the default nine-meter follow camera. The clearing
and original characters are visible immediately.

The forest is a new rendition of the source artwork: sampled animation poses
and vertex colors, leaf cutouts, flat terrain, and a bounded encounter. This is
not evidence of full texture fidelity, original terrain physics, a complete
fifth-edition game, or shared forest combat.

## Retained corrections

A final source review found that setting unchanged desktop HUD clearance
cleared a held click every frame. The setter now preserves pointer capture
unless the layout changes; the focused regression covers both cases.

## Limits

Simulator tests do not establish physical-device frame rate, memory pressure,
motion sensing, or battery use. Android compilation/JVM evidence is separate from a
new emulator or physical-device forest session. General Nostr scene discovery,
creator uploads, authoritative multiplayer rules, and the L1 construction zone
remain outside this implementation.
