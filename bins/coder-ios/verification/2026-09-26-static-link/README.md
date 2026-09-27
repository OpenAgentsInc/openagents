# iOS startup correction and mobile gestures

This folder retains the reproduced build 44 packaging failure and verification for
its build 45 replacement. The diagnosis and limits are in the
[assessment](../../../../docs/coder/verification/2026-09-26-ios-static-link.md).

## Original failure

- [Original executable launch failure](build44-missing-library-repro.log): the
  unchanged build 44 simulator app fails when its referenced external Rust dylib
  is unavailable. The library was restored after the check.
- [Bundle rejection](build44-bundle-rejection.log): the new verifier rejects the
  retained device archive's absolute Cargo library path.
- [Crash-report retrieval](crash-report-retrieval.json): no matching phone report
  was available through App Store Connect or local Xcode storage; the paired
  phone was offline. These facts do not establish absence of a device crash.

## Targeted implementation checks

- [Bundle verifier tests](bundle-verifier-tests.log): 11 passed.
- [Mobile gesture tests](rust-mobile-tests.log): 24 passed.
- [Shared camera tests](rust-camera-tests.log): 8 passed.
- [Shared runtime tests](rust-runtime-tests.log): 9 passed.
- [Strict Rust Clippy](rust-clippy.log): passed.
- [Android application and instrumentation Kotlin compilation](android-kotlin-compilation.log):
  passed. No new Android emulator acceptance is claimed.

The new normal-launch test uses no synthetic arguments or launch environment.
All [10 native tests passed](final-native-tests.log): normal launch/resume/cold
relaunch, fullscreen layout, motion look, touch fallback, double-tap jumping,
pinch in both directions without player or camera drift, computer reading and
pairing, and Gym boards. The separate synthetic tests exercise actual Metal
presentation and native gestures. The
[simulator bundle check](simulator-bundle-verification.json) passed against
Release Swift and optimized Rust.

A separate [normal launch](normal-launch.json) with both external Rust dylibs
temporarily hidden remained alive after 10 seconds. The libraries were restored
in cleanup. The [screenshot](normal-launch.png) shows the actual clean Verse
world; the simulator remains open. [Xcode summary](native-tests-summary.json).

[Build 45 distribution receipt](testflight-build45.json): upload succeeded,
processing is `VALID`, and internal distribution is `IN_BETA_TESTING`. Release
notes are saved. The [signed device archive](archive-bundle-verification.json)
passes the same dependency and signature checks. The receipt records the exact
clean source commit and archive hashes. Physical-device acceptance remains
unverified.

Text logs trim trailing whitespace for repository hygiene. Raw logs and Xcode
result bundles remain on the release machine. No model or benchmark runs were
performed.
