# Verse Gym evidence

These checks cover [issue #9700](https://github.com/OpenAgentsInc/openagents/issues/9700).
See the [assessment](../../../../docs/coder/verification/2026-09-26-verse-gym.md)
and [user guide](../../../../docs/verse/gym.md). No model, training, or benchmark
workload was run. Generated data and a harmless executable exercise the feature.

## Native app

[The native receipt](native-receipt.json) records the original nine-test iOS
suite (171.435 seconds), the final two-test Gym rerun (42.766 seconds), 17 mobile
Rust tests, strict Clippy, source hashes, and artifact hashes. The full suite
also covers the existing chat reader and Verse computer. The final Gym tests
cover actual touch movement through the doorway, board loading, metrics,
explicit recipe review, synthetic refusal, consumed confirmation, exit, and
background/resume. The simulator is iPhone 17 Pro on iOS 26.5.

The [original source snapshot](native-full-suite-source.json) and
[final source snapshot](native-final-source.json) distinguish the two native
builds. Host-only and desktop-only edits after these snapshots are excluded
from iOS; the release archive independently pins committed source.

- [Run list](native-runs.png): two labeled synthetic sources, progress, known
  and unavailable cost, and enabled preview recipe.
- [Metric chart](native-chart.png): actual fixture values, amber native chart,
  and an exact-value disclosure.
- [Recipe review](native-recipe-review.png) and
  [explicit preview refusal](native-preview-refusal.png).
- [Outside the Gym](native-outside.png): observation has stopped.

Some XCTest screenshots omit unchanged compositor layers. Native accessibility
assertions cover those labels; the receipt identifies the omissions. These are
simulator captures, not a physical iPhone display or camera claim.

The initial mobile unit run had 16 passes and one failed assertion that expected
preview acceptance. Preview deliberately refuses execution. The corrected
assertion requires refusal and no host receipt; [the earlier log](mobile-initial-tests.log)
is retained. The later 17-test run passes.

## Shared world and desktop

[The desktop receipt](desktop/receipt.json) identifies commands and artifacts.
The full all-targets command passed 132 library tests and the CLI test. A final
focused run passed 15 Gym library tests and the CLI test after the category
priority and consumed-review changes. Strict all-targets Clippy passed. The
chat/XP relay test binaries also completed; no production relay session is
claimed by this command without their opt-in relay environment.

Offline GPU capture produced and visually verified the actual shared geometry:
[exterior](desktop/gym-exterior.png), [interior](desktop/gym-interior.png), and
[synthetic board](desktop/gym-board.png). Capture exercises rendering, not
manual desktop keyboard interaction. The portable library test run passed 114
tests before the final confirmation refinement; the final focused desktop and
mobile checks cover that refinement.

## Host and relay

The host evidence is separate from the native preview. It exercises signed
connection grants, private encrypted messages on an authenticated loopback
WebSocket fixture, source projections, and a harmless local executable. It
checks expiry, revocation, exact-ID retry, restart uncertainty, changed
executables, source confinement, unavailable costs, and decimal round trips.
See the host logs and source receipt in `host/` for exact counts and checks.

## Distribution and limits

Build 41 uses the existing Coder bundle, signing team, version 0.5.0, and App
Store Connect app. Distribution is recorded separately after Apple's upload
and processing results are available. Native tests do not establish production
relay admission, a physical-device Gym session, thermal performance, or any
model's benchmark score. A configured host and separate device grant are
required for real boards; synthetic mode is never represented as live data.
