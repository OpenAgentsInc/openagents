# Homepage Playtest entry — 2026-10-30

Replaced Log in and Make your own with one **Playtest** button leading to
`/verse`: top right on desktop, full width above the feed on phones. Updated
the Coax hit targets, C layout identity and root/alias HTML/Markdown fallbacks.

Checks written before the change cover desktop clicks and phone taps reaching
the exact `/verse` route. All **61 homepage checks pass**. The linked Wasm
passes its 13 signed live-data fixture assertions and nine guarded renderer
frames at three viewport sizes. Visual rules and `git diff --check` pass.
Desktop and phone screenshots were inspected; only the two homepage goldens
were updated. No mutation or unrelated model gates were run.

Production verification is recorded after the staged release.
