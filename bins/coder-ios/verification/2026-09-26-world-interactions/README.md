# World interaction verification

The combined build is pending. [Design and controls](../../../../docs/verse/world-interactions.md)
track the ordered map, companion, and door issues. No physical-device or
TestFlight acceptance is claimed here until a distribution receipt is added.

- Shared route planner and controller tests cover collision clearance, bounded
  travel, manual cancellation, camera independence, and the real Gym doorway.
- All six map landmark targets have clear routes from spawn.
- Mobile Rust: 53 passed, 1 manual fixture ignored; strict Clippy passes.
- Desktop all-target compilation and Clippy pass.
- Swift application and UI-test typechecks pass. Android main and
  instrumentation compilation and two native JVM checks pass.
- Native map and pinch simulator acceptance is pending.
