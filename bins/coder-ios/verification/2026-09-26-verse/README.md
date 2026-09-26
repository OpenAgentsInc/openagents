# Coder iOS Verse verification: 2026-09-26

Coder `0.5.0 (39)` passed all six native UI tests on the iPhone 17 Pro
simulator running iOS 26.5. The app uses the real Rust static library and a
native Metal layer. The world and conversations are synthetic and offline.
No model call, benchmark, real transcript publication, or relay join ran.

The [receipt](receipt.json) records all six test durations, the simulator
executable digest, and 93 source hashes. Those source files were unchanged
between the final build and evidence retention. The final suite took 79.259
seconds. This is test duration, not a game performance measurement. The separate
[distribution receipt](testflight-build39.json) confirms build 39 is valid and
available in internal TestFlight from clean committed source.

## What passed

- The actual Metal surface presents frames from the shared Rust world.
- A real touch drag on the left side changes the player's X/Z position by
  more than 0.1 world units. Frame progress or gravity alone cannot pass this
  assertion. The retained movement screenshot shows Z at -0.83, compared with
  the spawn position of -10.
- Chats remains navigable after visiting Verse. Returning to Verse and
  backgrounding and foregrounding the app resumes frame presentation without
  a world error.
- The four existing reader checks still pass: chat selection and exact source,
  follow pause/resume, page pinning, and synthetic device identity across
  relaunches.
- The standalone Swift decoder checks accept the generic Surface reference
  and existing list/Markdown/opaque-intent shapes, and reject unknown elements.

Read the [final native test log](simulator-tests.log),
[decoder check result](native-contract-checks.log), and
[attachment manifest](attachments.json). The full `.xcresult` bundle remains
in the local worktree target directory named in the receipt.

## Screenshots

- [World after touch movement](synthetic-verse-movement.png).
- [Immediate XCTest foreground capture](synthetic-verse-resume.png).
- [Settled foreground capture](synthetic-verse-settled-resume.png).
- [Reader follow control](synthetic-follow.png).
- [Reader original source](synthetic-source.png).
- [Reader earlier-page navigation](synthetic-earlier-page.png).

The immediate XCTest foreground image omits some static native layers,
including the status bar and parts of labels, while updated counters and the
Metal world are visible. It is retained unchanged. A separate native
Simulator check backgrounded and reopened the same synthetic scene; after
foreground composition settled, an independent `simctl` capture showed all
labels, controls, tabs, and the status bar while frames advanced. This supports
a transient capture/composition explanation on this simulator, not a claim
that every foreground transition or physical device has been checked.

## Failure found and corrected

The [initial run](initial-simulator-tests.log) passed all four reader tests but
failed both Verse setups before the first frame. The native constructor
returned null, so the first screen could only report a startup failure.
Exposing the retained, sanitized constructor error identified the cause:
[the simulator's Metal device](device-limit-diagnostic.png) allowed 15
inter-stage shader variables, but the renderer requested the desktop default
of 16. The [diagnostic test log](device-limit-diagnostic.log) retains that
failed attempt.

The renderer now requests downlevel limits and the three shader varyings used
by the current scene, and validates those requirements against the adapter.
The [first corrected run](first-passing-simulator-tests.log) passed all six
tests. The suite then ran again after bounded GPU-buffer growth and final
mobile lifecycle changes; `simulator-tests.log` and the source hashes describe
that final result. Failed attempts were not replaced or counted as passes.

## Limits of this evidence

This receipt does not establish physical iPhone behavior, thermal or battery
performance, all supported iOS versions, accessible world navigation with
VoiceOver, locked-device identity behavior, mobile multiplayer interoperability,
or real phone-to-host reader pairing. It does not establish App Store
processing or TestFlight availability. No simulated result should be described
as a physical-device result.

The earlier [build 38 receipt](../2026-09-26/README.md) remains unchanged.
