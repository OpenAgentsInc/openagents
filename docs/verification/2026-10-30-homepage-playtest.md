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

Production release **`08adc36d54`** passed staging, visual/performance gates,
production starts/HTTP smoke and the ordinary live-data homepage gate.
Receipt: **`20261030T162723Z`**, staged/fast, 292.8 seconds; no held pages.

In muted headless Chrome on staging, a real mouse click at 1280×800 and a real
touch tap at 390×844 each navigated to `/verse`, where the Verse engine became
ready with no JS exceptions. Both homepage goldens matched with zero changed
blocks. Production refreshed all six projects with no JS exceptions. Independent
HTTP reads confirm `/` and `/new-land` contain the Playtest fallback and serve
the exact approved release Wasm (`bd089243007c5560e4d0348e8580f26c9f8930f0983564dc01471c923e2ab9ab`).

Compact evidence: [homepage Playtest checks](data/homepage-playtest-2026-10-30/).
