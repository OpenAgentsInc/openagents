# World interaction verification

The combined TestFlight build is pending. [Design and controls](../../../../docs/verse/world-interactions.md)
track the ordered map, companion, and door issues. No physical-device or
TestFlight acceptance is claimed here until a distribution receipt is added.

- Shared route planner and controller tests cover collision clearance, bounded
  travel, manual cancellation, camera independence, and the real Gym doorway.
- All six map landmark targets have clear routes from spawn.
- Mobile Rust at the map checkpoint: 54 passed, 1 manual fixture ignored; strict Clippy passes.
- Desktop all-target compilation and Clippy pass.
- Swift application and UI-test typechecks pass. Android main and
  instrumentation compilation and two native JVM checks pass.
- Both real-coordinate iOS map tests pass: destination selection/manual
  cancellation, and map drag isolation. The accompanying full-screen pinch
  test initially failed because its zoom-out contact landed on Recenter;
  the retained log includes that failure. The clear-surface retest passes in both directions.

At the companion checkpoint, the shared Verse suite passes 146 tests and
the mobile suite passes 56 tests (one external fixture ignored). Affected
all-target Clippy and the desktop click regression pass. Four combined native
tests pass: companion real tap, both map tests, and pinch in both directions.
