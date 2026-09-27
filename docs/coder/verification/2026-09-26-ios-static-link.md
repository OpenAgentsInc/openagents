# iOS startup and gesture correction — September 26, 2026

Build 44 could exit before the application initialized because its executable
required a Rust dynamic library at an absolute path on the build computer.
The replacement links the Rust static archive explicitly and checks the
completed app bundle for external library dependencies. It also removes the
world's title and idle-status overlays, replaces the jump button with a double
tap, and replaces zoom buttons with a two-finger pinch.

Build **0.5.0 (45)** is the replacement in preparation. All 10 final native
simulator tests passed with Release Swift and optimized Rust. TestFlight
delivery is pending. The
[evidence directory](../../../bins/coder-ios/verification/2026-09-26-static-link/README.md)
is the release record; a completed build or upload is not a physical-device
launch observation.

## Failure evidence

No physical-phone crash stack was available during this investigation. The
phone was unavailable for log retrieval, and App Store Connect had no crash
feedback report to retrieve. The diagnosis comes from the retained build 44
binary and a local reproduction, not an inferred stack trace.

The iOS host linked Rust with `-lcoder_mobile`. After Android support added a
`cdylib` output beside the static archive, that ambiguous linker argument could
select `libcoder_mobile.dylib`. The resulting executable recorded the dynamic
library's absolute Cargo build path. Builds 43 and 44 were therefore dependent
on a file outside their app bundles.

The retained build 44 simulator app loaded while that file existed on its
build computer. Temporarily moving the referenced library away reproduced an
immediate `dyld` failure with `Library not loaded`. A phone has no such build
path. The previous simulator checks and App Store Connect processing result
therefore did not establish that the delivered app could start independently.
The [motion-camera record](2026-09-26-motion-camera.md) now identifies this
limitation in build 44's earlier verification.

## Packaging correction

The Xcode project names `$(CODER_RUST_LIBRARY_DIR)/libcoder_mobile.a` explicitly.
The Rust crate retains its Android dynamic-library output; the iOS host chooses
its static input without relying on linker search order.

[`scripts/verify-coder-ios-bundle.py`](../../../scripts/verify-coder-ios-bundle.py)
checks every Mach-O image in a completed app, including embedded libraries
that the main executable does not currently load. It verifies signatures,
rejects a dynamic `coder_mobile` dependency, resolves bundled dependency chains
and runpaths, and rejects missing libraries, build-machine paths, and symlinks
that escape the bundle. System libraries remain permitted. This checks the
packaged executable rather than only the project's linker settings.

[`scripts/build-coder-mobile.sh`](../../../scripts/build-coder-mobile.sh)
runs the bundle check for simulator and device builds, archives, and locally
exported IPAs. `CODER_IOS_RUST_PROFILE=release` also permits simulator checks
with optimized Rust; device builds and archives continue to use release Rust.
The archive retains its source commit, working-tree status, dependency-lock
hash, compiler versions, executable hash, and bundle-verification receipt.

## World controls

The mobile world removes the **Coder** heading, world/idle-status text, and
walk/sprint, jump, and zoom buttons. The camera-mode control, motion recenter,
nearby interactions, and actionable errors remain available. Synthetic test
metadata is exposed through the surface's accessibility value instead of
visible diagnostic labels.

Double-tap recognition lives in `coder-mobile`. Each tap must finish within
250 ms, move no more than 12 logical points, and finish near the preceding tap
within 350 ms and 32 logical points. The second release submits one jump to
the shared Verse controller. Timing uses Rust's monotonic receipt clock, so a
stalled render frame cannot make a long hold look like a short tap. The monitor
retains its immediate single-tap action and cannot also trigger a jump.

Dragging, a cancelled touch, malformed coordinates, a long hold, or a second
finger discards pending taps. Recenter, camera-mode changes, panel transitions,
backgrounding, and viewport changes also discard them. Pinching cancels held
pointer input and pending jumps before changing camera distance. Native
adapters supply an incremental finger-separation scale; Rust applies its
inverse to camera distance and keeps the existing 2.5–40 meter bounds. This
application behavior belongs to Coder and Verse; Rust Native remains a generic
UI crate.

## Completed checks

| Check | Result |
| --- | --- |
| `cargo test -p coder-mobile --lib verse_app::tests` | 24 passed, including double-tap acceptance, invalid gestures, stalled-frame timing, lifecycle resets, pinch bounds, and existing motion/monitor behavior |
| `cargo test -p verse --no-default-features --lib camera::tests` | 8 passed |
| `cargo test -p verse --no-default-features --lib runtime::tests` | 9 passed |
| `cargo clippy -p coder-mobile --lib --tests -- -D warnings` | Passed |
| Formatting and whitespace checks for the changed Rust files | Passed |
| iOS Release simulator tests | 10 passed: normal launch, resume, relaunch, fullscreen, motion, touch fallback, double-tap, pinch, computer, and Gym |
| Android application and instrumentation Kotlin compilation | Passed; no new emulator acceptance claimed |
| Packaging verifier regression tests | 11 passed, including the build 44 absolute-library failure and transitive bundled dependencies |

Final simulator launch, native gesture, archive, and delivery evidence is
recorded in the [release evidence directory](../../../bins/coder-ios/verification/2026-09-26-static-link/README.md)
when complete. Physical-device launch, motion feel, and frame rate remain
separate observations; no unavailable phone log is presented as collected
crash evidence.
